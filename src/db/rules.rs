use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

use crate::model::{GlossaryDefinition, RuleEntryOut, RuleWithChildren};
use crate::rules::parser::{GlossaryEntry, RuleEntry, SectionEntry};

#[derive(Debug)]
pub struct RulesDb {
    conn: Connection,
}

impl RulesDb {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() {
            bail!(
                "Base règles introuvable ({}). Lancez `kb update rules` pour la télécharger.",
                path.display()
            );
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("impossible d'ouvrir la base règles {}", path.display()))?;
        Ok(Self { conn })
    }

    pub fn build(
        path: &Path,
        version: &str,
        effective_date: &str,
        sections: &[SectionEntry],
        rules: &[RuleEntry],
        glossary: &[GlossaryEntry],
    ) -> Result<()> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE meta (version TEXT, effective_date TEXT);
             CREATE TABLE sections (number TEXT PRIMARY KEY, title TEXT NOT NULL);
             CREATE TABLE rules (number TEXT PRIMARY KEY, parent TEXT, text TEXT NOT NULL);
             CREATE VIRTUAL TABLE rules_fts USING fts5(number, text);
             CREATE TABLE glossary (term TEXT PRIMARY KEY, definition TEXT NOT NULL);",
        )?;

        conn.execute(
            "INSERT INTO meta (version, effective_date) VALUES (?1, ?2)",
            (version, effective_date),
        )?;
        for section in sections {
            // Le sommaire répète les en-têtes de Section.
            conn.execute(
                "INSERT OR IGNORE INTO sections (number, title) VALUES (?1, ?2)",
                (&section.number, &section.title),
            )?;
        }
        for rule in rules {
            let inserted = conn.execute(
                "INSERT OR IGNORE INTO rules (number, parent, text) VALUES (?1, ?2, ?3)",
                (&rule.number, &rule.parent, &rule.text),
            )?;
            if inserted > 0 {
                conn.execute(
                    "INSERT INTO rules_fts (number, text) VALUES (?1, ?2)",
                    (&rule.number, &rule.text),
                )?;
            }
        }
        for entry in glossary {
            conn.execute(
                "INSERT OR REPLACE INTO glossary (term, definition) VALUES (?1, ?2)",
                (&entry.term, &entry.definition),
            )?;
        }
        Ok(())
    }

    pub fn version(&self) -> Result<Option<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT version, effective_date FROM meta LIMIT 1")?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some((row.get(0)?, row.get(1)?))),
            None => Ok(None),
        }
    }

    /// Enfants d'une Section "100" : "100.*" ; d'une Règle "100.1" : "100.1[a-z]".
    pub fn rule_by_number(&self, number: &str) -> Result<Option<RuleWithChildren>> {
        let section_title: Option<String> = self
            .conn
            .query_row(
                "SELECT title FROM sections WHERE number = ?1",
                [number],
                |row| row.get(0),
            )
            .ok();

        let rule_text: Option<String> = self
            .conn
            .query_row(
                "SELECT text FROM rules WHERE number = ?1",
                [number],
                |row| row.get(0),
            )
            .ok();

        if section_title.is_none() && rule_text.is_none() {
            return Ok(None);
        }

        let mut stmt = self
            .conn
            .prepare("SELECT number, text FROM rules WHERE number LIKE ?1 ORDER BY number")?;
        let prefix = format!("{number}.%");
        let mut children: Vec<RuleEntryOut> = stmt
            .query_map([&prefix], |row| {
                Ok(RuleEntryOut {
                    number: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        if section_title.is_none() {
            let mut letter_stmt = self
                .conn
                .prepare("SELECT number, text FROM rules WHERE number GLOB ?1 ORDER BY number")?;
            let glob = format!("{number}[a-z]");
            let letter_children: Vec<RuleEntryOut> = letter_stmt
                .query_map([&glob], |row| {
                    Ok(RuleEntryOut {
                        number: row.get(0)?,
                        text: row.get(1)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            children = letter_children;
        }

        Ok(Some(RuleWithChildren {
            number: number.to_string(),
            title: section_title,
            text: rule_text,
            children,
        }))
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<RuleEntryOut>> {
        // Recherche de phrase : la syntaxe d'opérateurs FTS5 n'est pas exposée.
        let phrase = format!("\"{}\"", query.replace('"', "\"\""));
        let mut stmt = self.conn.prepare(
            "SELECT number, text FROM rules_fts WHERE rules_fts MATCH ?1 \
             ORDER BY rank LIMIT ?2",
        )?;
        let results = stmt
            .query_map((phrase, limit as i64), |row| {
                Ok(RuleEntryOut {
                    number: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(results)
    }

    pub fn define(&self, term: &str) -> Result<Option<GlossaryDefinition>> {
        let mut stmt = self.conn.prepare(
            "SELECT term, definition FROM glossary WHERE term = ?1 COLLATE NOCASE LIMIT 1",
        )?;
        let mut rows = stmt.query([term])?;
        match rows.next()? {
            Some(row) => Ok(Some(GlossaryDefinition {
                term: row.get(0)?,
                definition: row.get(1)?,
            })),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.sqlite");
        let sections = vec![SectionEntry {
            number: "100".to_string(),
            title: "General".to_string(),
        }];
        let rules = vec![
            RuleEntry {
                number: "100.1".to_string(),
                parent: Some("100".to_string()),
                text: "These Magic rules apply to any Magic game.".to_string(),
            },
            RuleEntry {
                number: "100.1a".to_string(),
                parent: Some("100.1".to_string()),
                text: "A two-player game is a game with two players.".to_string(),
            },
            RuleEntry {
                number: "100.2".to_string(),
                parent: Some("100".to_string()),
                text: "Players need a deck of traditional Magic cards.".to_string(),
            },
        ];
        let glossary = vec![GlossaryEntry {
            term: "Ability".to_string(),
            definition: "A characteristic granted to a player or permanent.".to_string(),
        }];
        RulesDb::build(
            &path,
            "20260227",
            "February 27, 2026",
            &sections,
            &rules,
            &glossary,
        )
        .unwrap();
        (dir, path)
    }

    #[test]
    fn stores_and_reads_version() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        let (version, effective_date) = db.version().unwrap().unwrap();
        assert_eq!(version, "20260227");
        assert_eq!(effective_date, "February 27, 2026");
    }

    #[test]
    fn section_lookup_returns_all_descendants() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        let section = db.rule_by_number("100").unwrap().expect("section found");
        assert_eq!(section.title, Some("General".to_string()));
        let numbers: Vec<_> = section.children.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(numbers, vec!["100.1", "100.1a", "100.2"]);
    }

    #[test]
    fn rule_lookup_returns_only_lettered_children() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        let rule = db.rule_by_number("100.1").unwrap().expect("rule found");
        assert!(rule.text.unwrap().starts_with("These Magic rules"));
        let numbers: Vec<_> = rule.children.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(numbers, vec!["100.1a"]);
    }

    #[test]
    fn unknown_number_returns_none() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        assert!(db.rule_by_number("999").unwrap().is_none());
    }

    #[test]
    fn full_text_search_finds_matching_rules() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        let results = db.search("two-player", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].number, "100.1a");
    }

    #[test]
    fn define_is_case_insensitive() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        let def = db.define("ability").unwrap().expect("term found");
        assert_eq!(def.term, "Ability");
    }

    #[test]
    fn define_unknown_term_returns_none() {
        let (_dir, path) = build_fixture();
        let db = RulesDb::open(&path).unwrap();
        assert!(db.define("Nonexistent").unwrap().is_none());
    }

    #[test]
    fn missing_db_file_errors_with_update_hint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rules.sqlite");
        let err = RulesDb::open(&path).unwrap_err();
        assert!(err.to_string().contains("kb update rules"));
    }
}
