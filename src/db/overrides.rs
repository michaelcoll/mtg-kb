use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rusqlite::Connection;
use serde::Serialize;

use crate::model::CardCorrections;

/// Corrections par nom complet de Carte.
pub type Corrections = HashMap<String, CardCorrections>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CorrectionField {
    Role,
    Theme,
    Legality,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CorrectionValue {
    Roles(Vec<String>),
    Themes(Vec<String>),
    LegalInCommander(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionEntry {
    pub card: String,
    pub field: String,
    pub value: serde_json::Value,
    pub reason: String,
    pub date: String,
}

#[derive(Debug)]
pub struct OverridesDb {
    conn: Connection,
}

impl CorrectionField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Role => "role",
            Self::Theme => "theme",
            Self::Legality => "legality",
        }
    }
}

impl CorrectionValue {
    fn field(&self) -> CorrectionField {
        match self {
            Self::Roles(_) => CorrectionField::Role,
            Self::Themes(_) => CorrectionField::Theme,
            Self::LegalInCommander(_) => CorrectionField::Legality,
        }
    }

    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Roles(labels) | Self::Themes(labels) => serde_json::json!(labels),
            Self::LegalInCommander(true) => serde_json::json!("legal"),
            Self::LegalInCommander(false) => serde_json::json!("banned"),
        }
    }
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS corrections (
    card TEXT NOT NULL,
    field TEXT NOT NULL CHECK (field IN ('role', 'theme', 'legality')),
    value TEXT NOT NULL,
    reason TEXT NOT NULL,
    date TEXT NOT NULL,
    PRIMARY KEY (card, field)
)";

impl OverridesDb {
    /// `None` si la base n'existe pas encore : la lecture ne la crée pas.
    pub fn open(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("impossible d'ouvrir la base {}", path.display()))?;
        Ok(Some(Self { conn }))
    }

    pub fn create(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir la base {}", path.display()))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Remplace la Correction existante du même champ.
    pub fn set(&self, card: &str, value: &CorrectionValue, reason: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO corrections (card, field, value, reason, date) \
             VALUES (?1, ?2, ?3, ?4, date('now', 'localtime'))",
            (
                card,
                value.field().as_str(),
                value.to_json().to_string(),
                reason,
            ),
        )?;
        Ok(())
    }

    /// `false` si aucune Correction n'existait.
    pub fn remove(&self, card: &str, field: CorrectionField) -> Result<bool> {
        let deleted = self.conn.execute(
            "DELETE FROM corrections WHERE card = ?1 AND field = ?2",
            (card, field.as_str()),
        )?;
        Ok(deleted > 0)
    }

    pub fn list(&self) -> Result<Vec<CorrectionEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT card, field, value, reason, date FROM corrections \
             ORDER BY card, CASE field WHEN 'role' THEN 0 WHEN 'theme' THEN 1 ELSE 2 END",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(card, field, value, reason, date)| {
                let value = serde_json::from_str(&value).with_context(|| {
                    format!("Correction illisible pour « {card} » ({field}) : {value}")
                })?;
                Ok(CorrectionEntry {
                    card,
                    field,
                    value,
                    reason,
                    date,
                })
            })
            .collect()
    }

    pub fn corrections(&self) -> Result<Corrections> {
        let mut corrections = Corrections::new();
        for entry in self.list()? {
            let unreadable = || {
                anyhow!(
                    "Correction illisible pour « {} » ({}) : {}",
                    entry.card,
                    entry.field,
                    entry.value
                )
            };
            let labels = || -> Result<Vec<String>> {
                serde_json::from_value(entry.value.clone()).map_err(|_| unreadable())
            };
            let correction = corrections.entry(entry.card.clone()).or_default();
            match entry.field.as_str() {
                "role" => correction.roles = Some(labels()?),
                "theme" => correction.themes = Some(labels()?),
                _ => {
                    correction.legal_in_commander = Some(match entry.value.as_str() {
                        Some("legal") => true,
                        Some("banned") => false,
                        _ => return Err(unreadable()),
                    })
                }
            }
        }
        Ok(corrections)
    }
}

/// Aucune Correction si la base est absente.
pub fn load_corrections(path: &Path) -> Result<Corrections> {
    match OverridesDb::open(path)? {
        Some(db) => db.corrections(),
        None => Ok(Corrections::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(values: &[&str]) -> CorrectionValue {
        CorrectionValue::Roles(values.iter().map(|v| v.to_string()).collect())
    }

    fn created() -> (tempfile::TempDir, OverridesDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = OverridesDb::create(&dir.path().join("overrides.sqlite")).unwrap();
        (dir, db)
    }

    #[test]
    fn a_missing_base_means_no_corrections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overrides.sqlite");
        assert!(load_corrections(&path).unwrap().is_empty());
        assert!(OverridesDb::open(&path).unwrap().is_none());
        assert!(!path.exists(), "reading must not create the base");
    }

    #[test]
    fn create_makes_the_base_on_first_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("overrides.sqlite");
        OverridesDb::create(&path)
            .unwrap()
            .set("Bite Down", &roles(&["removal_cible"]), "bite")
            .unwrap();
        let corrections = load_corrections(&path).unwrap();
        assert_eq!(
            corrections["Bite Down"].roles,
            Some(vec!["removal_cible".to_string()])
        );
    }

    #[test]
    fn setting_a_field_again_replaces_it() {
        let (_dir, db) = created();
        db.set("Wild Growth", &roles(&["pioche"]), "erreur")
            .unwrap();
        db.set("Wild Growth", &roles(&["ramp"]), "aura de mana")
            .unwrap();
        let entries = db.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].value, serde_json::json!(["ramp"]));
        assert_eq!(entries[0].reason, "aura de mana");
    }

    #[test]
    fn fields_of_one_card_are_independent() {
        let (_dir, db) = created();
        db.set("Drudge Spell", &roles(&[]), "ses propres tokens")
            .unwrap();
        db.set(
            "Drudge Spell",
            &CorrectionValue::Themes(vec!["tokens".to_string()]),
            "tokens",
        )
        .unwrap();
        db.set(
            "Drudge Spell",
            &CorrectionValue::LegalInCommander(false),
            "test",
        )
        .unwrap();
        let corrections = db.corrections().unwrap();
        assert_eq!(
            corrections["Drudge Spell"],
            CardCorrections {
                roles: Some(vec![]),
                themes: Some(vec!["tokens".to_string()]),
                legal_in_commander: Some(false),
            }
        );
    }

    #[test]
    fn remove_reverts_one_field_to_detection() {
        let (_dir, db) = created();
        db.set("Leveler", &roles(&[]), "exil de ses cartes")
            .unwrap();
        db.set("Leveler", &CorrectionValue::LegalInCommander(true), "test")
            .unwrap();
        assert!(db.remove("Leveler", CorrectionField::Role).unwrap());
        assert!(!db.remove("Leveler", CorrectionField::Role).unwrap());
        let corrections = db.corrections().unwrap();
        assert_eq!(corrections["Leveler"].roles, None);
        assert_eq!(corrections["Leveler"].legal_in_commander, Some(true));
    }

    #[test]
    fn list_shows_card_field_value_reason_and_date() {
        let (_dir, db) = created();
        db.set("Bite Down", &roles(&["removal_cible"]), "bite")
            .unwrap();
        db.set(
            "Black Lotus",
            &CorrectionValue::LegalInCommander(false),
            "test",
        )
        .unwrap();
        let entries = db.list().unwrap();
        assert_eq!(entries[0].card, "Bite Down");
        assert_eq!(entries[0].field, "role");
        assert_eq!(entries[0].reason, "bite");
        assert_eq!(entries[0].date.len(), "2026-09-23".len());
        assert_eq!(entries[1].field, "legality");
        assert_eq!(entries[1].value, serde_json::json!("banned"));
    }
}
