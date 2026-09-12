use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

use crate::model::{Card, split_csv_field};

#[derive(Debug)]
pub struct CardsDb {
    conn: Connection,
}

impl CardsDb {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() {
            bail!(
                "Base cartes introuvable ({}). Lancez `kb update` pour la télécharger.",
                path.display()
            );
        }
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .with_context(|| format!("impossible d'ouvrir la base cartes {}", path.display()))?;
        Ok(Self { conn })
    }

    /// Résout une Carte par son nom oracle exact. Dédoublonne les Impressions
    /// (plusieurs lignes `cards` peuvent partager le même nom) en ne retenant
    /// que la première trouvée : les champs oracle sont stables entre
    /// Impressions.
    pub fn card_by_name(&self, name: &str) -> Result<Option<Card>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, manaCost, manaValue, type, types, subtypes, supertypes, \
             text, colorIdentity, colors, keywords, power, toughness, loyalty \
             FROM cards WHERE name = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Card {
                name: row.get(0)?,
                mana_cost: row.get(1)?,
                mana_value: row.get(2)?,
                type_line: row.get(3)?,
                types: split_csv_field(row.get::<_, Option<String>>(4)?.as_deref()),
                subtypes: split_csv_field(row.get::<_, Option<String>>(5)?.as_deref()),
                supertypes: split_csv_field(row.get::<_, Option<String>>(6)?.as_deref()),
                oracle_text: row.get(7)?,
                color_identity: split_csv_field(row.get::<_, Option<String>>(8)?.as_deref()),
                colors: split_csv_field(row.get::<_, Option<String>>(9)?.as_deref()),
                keywords: split_csv_field(row.get::<_, Option<String>>(10)?.as_deref()),
                power: row.get(11)?,
                toughness: row.get(12)?,
                loyalty: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_db() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                setCode TEXT
            );
            INSERT INTO cards VALUES (
                'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact', NULL, NULL,
                '{T}: Add {C}{C}.', NULL, NULL, NULL, NULL, NULL, NULL, 'LEA'
            );
            INSERT INTO cards VALUES (
                'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact', NULL, NULL,
                '{T}: Add {C}{C}.', NULL, NULL, NULL, NULL, NULL, NULL, 'C21'
            );
            INSERT INTO cards VALUES (
                'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature',
                'Phyrexian, Angel, Horror', 'Legendary', 'Flying, vigilance...',
                'B, G, U, W', 'W, U, B, G', 'Deathtouch, Flying, Lifelink, Vigilance, Proliferate',
                '4', '4', NULL, 'M15'
            );
            "#,
        )
        .unwrap();
        (dir, path)
    }

    #[test]
    fn resolves_card_by_exact_name_deduped_across_printings() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let card = db.card_by_name("Sol Ring").unwrap().expect("card found");
        assert_eq!(card.name, "Sol Ring");
        assert_eq!(card.mana_value, Some(1.0));
    }

    #[test]
    fn splits_multi_valued_fields() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let card = db
            .card_by_name("Atraxa, Praetors' Voice")
            .unwrap()
            .expect("card found");
        assert_eq!(card.color_identity, vec!["B", "G", "U", "W"]);
        assert_eq!(card.subtypes, vec!["Phyrexian", "Angel", "Horror"]);
        assert_eq!(card.keywords.len(), 5);
    }

    #[test]
    fn unknown_card_returns_none() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert!(db.card_by_name("Not A Real Card").unwrap().is_none());
    }

    #[test]
    fn missing_db_file_errors_with_update_hint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let err = CardsDb::open(&path).unwrap_err();
        assert!(err.to_string().contains("kb update"));
    }
}
