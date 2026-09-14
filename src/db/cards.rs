use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, Row};

use crate::model::{Card, ReferencePrinting, Ruling, SetInfo, split_csv_field};

/// Colonnes de `cardLegalities` acceptées comme filtre de format : whitelist
/// stricte, car le nom de colonne est injecté tel quel dans le SQL.
const LEGALITY_FORMATS: &[&str] = &[
    "alchemy",
    "brawl",
    "commander",
    "competitivebrawl",
    "duel",
    "future",
    "gladiator",
    "historic",
    "legacy",
    "modern",
    "oathbreaker",
    "oldschool",
    "pauper",
    "paupercommander",
    "penny",
    "pioneer",
    "predh",
    "premodern",
    "standard",
    "standardbrawl",
    "timeless",
    "tlr",
    "vintage",
];

#[derive(Debug, Default, Clone)]
pub struct SearchFilters {
    pub name: Option<String>,
    pub type_contains: Option<String>,
    pub subtype_contains: Option<String>,
    /// Sous-chaînes du texte oracle, combinées en ET (une Carte doit
    /// contenir chacune d'elles).
    pub oracle_text_contains: Vec<String>,
    pub color_identity_subset_of: Option<Vec<String>>,
    pub legal_in_format: Option<String>,
    pub mana_value: Option<f64>,
    /// Mana value minimale (borne incluse).
    pub mana_value_min: Option<f64>,
    /// Mana value maximale (borne incluse).
    pub mana_value_max: Option<f64>,
    /// Nombre maximal de résultats. `0` signifie « sans limite » (tout le
    /// pool correspondant aux filtres) : utilisé par l'analyse de Candidats,
    /// qui doit couvrir toute la Base cartes plutôt qu'un top alphabétique.
    pub limit: usize,
}

const CARD_COLUMNS: &str = "name, manaCost, manaValue, type, types, subtypes, supertypes, \
             text, colorIdentity, colors, keywords, power, toughness, loyalty";

fn card_from_row(row: &Row) -> rusqlite::Result<Card> {
    Ok(Card {
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
    })
}

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
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("impossible d'ouvrir la base cartes {}", path.display()))?;
        Ok(Self { conn })
    }

    /// Résout une Carte par son nom oracle exact. Dédoublonne les Impressions
    /// (plusieurs lignes `cards` peuvent partager le même nom) en ne retenant
    /// que la première trouvée : les champs oracle sont stables entre
    /// Impressions.
    pub fn card_by_name(&self, name: &str) -> Result<Option<Card>> {
        let sql = format!("SELECT {CARD_COLUMNS} FROM cards WHERE name = ?1 LIMIT 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query([name])?;
        match rows.next()? {
            Some(row) => Ok(Some(card_from_row(row)?)),
            None => Ok(None),
        }
    }

    /// Recherche des Cartes selon des filtres combinés, dédoublonnées par nom
    /// oracle. Les filtres appliqués en SQL (nom, type, texte, mana value,
    /// légalité) réduisent le jeu de candidats ; le filtre d'Identité de
    /// couleur (sous-ensemble) est appliqué ensuite car il n'est pas
    /// exprimable simplement sur la colonne texte `colorIdentity`.
    pub fn search(&self, filters: &SearchFilters) -> Result<Vec<Card>> {
        let mut sql = format!("SELECT DISTINCT {CARD_COLUMNS} FROM cards c");
        let mut joins = String::new();
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(format) = &filters.legal_in_format {
            let column = LEGALITY_FORMATS
                .iter()
                .find(|f| f.eq_ignore_ascii_case(format))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "format inconnu « {} » (formats valides : {})",
                        format,
                        LEGALITY_FORMATS.join(", ")
                    )
                })?;
            joins.push_str(" JOIN cardLegalities cl ON cl.uuid = c.uuid");
            conditions.push(format!("cl.{column} = 'Legal'"));
        }
        if let Some(name) = &filters.name {
            conditions.push("c.name LIKE ?".to_string());
            params.push(Box::new(format!("%{name}%")));
        }
        if let Some(type_contains) = &filters.type_contains {
            conditions.push("c.type LIKE ?".to_string());
            params.push(Box::new(format!("%{type_contains}%")));
        }
        if let Some(subtype_contains) = &filters.subtype_contains {
            conditions.push("c.subtypes LIKE ?".to_string());
            params.push(Box::new(format!("%{subtype_contains}%")));
        }
        for text in &filters.oracle_text_contains {
            conditions.push("c.text LIKE ?".to_string());
            params.push(Box::new(format!("%{text}%")));
        }
        if let Some(mv) = filters.mana_value {
            conditions.push("c.manaValue = ?".to_string());
            params.push(Box::new(mv));
        }
        if let Some(min) = filters.mana_value_min {
            conditions.push("c.manaValue >= ?".to_string());
            params.push(Box::new(min));
        }
        if let Some(max) = filters.mana_value_max {
            conditions.push("c.manaValue <= ?".to_string());
            params.push(Box::new(max));
        }

        sql.push_str(&joins);
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY c.name");
        // `limit == 0` signifie « sans limite » : pas de LIMIT SQL, pas de
        // troncature côté Rust (utilisé pour couvrir tout le pool de
        // Candidats, cf. CONTEXT.md). Sinon, sur-échantillonne avant le
        // filtre d'Identité de couleur, appliqué en Rust, pour ne pas
        // tronquer prématurément les résultats.
        if filters.limit > 0 {
            let fetch_cap = filters.limit.saturating_mul(20).max(500);
            sql.push_str(&format!(" LIMIT {fetch_cap}"));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), card_from_row)?;

        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for row in rows {
            let card = row?;
            if !seen.insert(card.name.clone()) {
                continue;
            }
            if let Some(allowed) = &filters.color_identity_subset_of
                && !card
                    .color_identity
                    .iter()
                    .all(|c| allowed.iter().any(|a| a.eq_ignore_ascii_case(c)))
            {
                continue;
            }
            results.push(card);
            if filters.limit > 0 && results.len() >= filters.limit {
                break;
            }
        }
        Ok(results)
    }

    pub fn set_by_code(&self, code: &str) -> Result<Option<SetInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT code, name, releaseDate, type, block, baseSetSize, totalSetSize \
             FROM sets WHERE code = ?1 COLLATE NOCASE LIMIT 1",
        )?;
        let mut rows = stmt.query([code])?;
        match rows.next()? {
            Some(row) => Ok(Some(SetInfo {
                code: row.get(0)?,
                name: row.get(1)?,
                release_date: row.get(2)?,
                set_type: row.get(3)?,
                block: row.get(4)?,
                base_set_size: row.get(5)?,
                total_set_size: row.get(6)?,
            })),
            None => Ok(None),
        }
    }

    /// Rulings datés d'une Carte par nom oracle exact. Utilise l'uuid de la
    /// première Impression trouvée : les Rulings d'une Carte sont identiques
    /// entre Impressions.
    pub fn rulings_by_name(&self, name: &str) -> Result<Option<Vec<Ruling>>> {
        let mut uuid_stmt = self
            .conn
            .prepare("SELECT uuid FROM cards WHERE name = ?1 LIMIT 1")?;
        let mut uuid_rows = uuid_stmt.query([name])?;
        let Some(row) = uuid_rows.next()? else {
            return Ok(None);
        };
        let uuid: String = row.get(0)?;
        drop(uuid_rows);

        let mut stmt = self
            .conn
            .prepare("SELECT date, text FROM cardRulings WHERE uuid = ?1 ORDER BY date, rowid")?;
        let rulings = stmt
            .query_map([uuid], |row| {
                Ok(Ruling {
                    date: row.get(0)?,
                    text: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some(rulings))
    }

    /// Légalité en Commander d'une Carte par nom oracle exact. `None` si la
    /// Carte n'existe pas dans la Base cartes.
    ///
    /// `cardLegalities` contient une ligne par Impression (uuid) d'une même
    /// Carte, et certaines Impressions (promos notamment) n'ont pas de
    /// légalité renseignée (`commander` à NULL) alors que d'autres
    /// Impressions de la même Carte sont bien légales. On considère donc la
    /// Carte légale dès qu'**au moins une** Impression l'est, plutôt que de
    /// se fier à une seule ligne arbitraire (`LIMIT 1` sans `ORDER BY`).
    pub fn is_legal_commander(&self, name: &str) -> Result<Option<bool>> {
        if self.card_by_name(name)?.is_none() {
            return Ok(None);
        }

        let mut stmt = self.conn.prepare(
            "SELECT EXISTS( \
                 SELECT 1 FROM cards c \
                 JOIN cardLegalities cl ON cl.uuid = c.uuid \
                 WHERE c.name = ?1 AND cl.commander = 'Legal' \
             )",
        )?;
        let legal: bool = stmt.query_row([name], |row| row.get(0))?;
        Ok(Some(legal))
    }

    /// Résout l'Impression de référence d'une Carte (voir CONTEXT.md) : la
    /// plus récente en papier, hors promo, hors format surdimensionné et
    /// hors cartes fantaisie. À défaut, relâche d'abord le filtre promo,
    /// puis les filtres surdimensionné/fantaisie. `None` si aucune
    /// Impression papier n'a de `scryfallId`.
    pub fn reference_printing(&self, name: &str) -> Result<Option<ReferencePrinting>> {
        const TIERS: &[&str] = &[
            "AND (c.isPromo = 0 OR c.isPromo IS NULL) \
             AND (c.isOversized = 0 OR c.isOversized IS NULL) \
             AND (c.isFunny = 0 OR c.isFunny IS NULL)",
            "AND (c.isOversized = 0 OR c.isOversized IS NULL) \
             AND (c.isFunny = 0 OR c.isFunny IS NULL)",
            "",
        ];
        for extra_filter in TIERS {
            let sql = format!(
                "SELECT c.setCode, c.number, ci.scryfallId, c.layout FROM cards c \
                 JOIN cardIdentifiers ci ON ci.uuid = c.uuid \
                 LEFT JOIN sets s ON s.code = c.setCode \
                 WHERE c.name = ?1 AND c.availability LIKE '%paper%' \
                 AND ci.scryfallId IS NOT NULL {extra_filter} \
                 ORDER BY s.releaseDate DESC LIMIT 1"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query([name])?;
            if let Some(row) = rows.next()? {
                let layout: Option<String> = row.get(3)?;
                return Ok(Some(ReferencePrinting {
                    set_code: row.get(0)?,
                    number: row.get(1)?,
                    scryfall_id: row.get(2)?,
                    is_two_faced: matches!(layout.as_deref(), Some("transform" | "modal_dfc")),
                }));
            }
        }
        Ok(None)
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
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                setCode TEXT, number TEXT, availability TEXT,
                isPromo BOOLEAN, isOversized BOOLEAN, isFunny BOOLEAN, layout TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT, standard TEXT);
            CREATE TABLE cardRulings (uuid TEXT, date TEXT, text TEXT);
            CREATE TABLE cardIdentifiers (uuid TEXT, scryfallId TEXT);
            CREATE TABLE sets (
                code TEXT, name TEXT, releaseDate TEXT, type TEXT, block TEXT,
                baseSetSize INTEGER, totalSetSize INTEGER
            );

            INSERT INTO cards VALUES (
                'sol-lea', 'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact', NULL, NULL,
                '{T}: Add {C}{C}.', NULL, NULL, NULL, NULL, NULL, NULL, 'LEA',
                '1', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'sol-c21', 'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact', NULL, NULL,
                '{T}: Add {C}{C}.', NULL, NULL, NULL, NULL, NULL, NULL, 'C21',
                '263', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature',
                'Phyrexian, Angel, Horror', 'Legendary', 'Flying, vigilance...',
                'B, G, U, W', 'W, U, B, G', 'Deathtouch, Flying, Lifelink, Vigilance, Proliferate',
                '4', '4', NULL, 'M15', '1', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'llanowar', 'Llanowar Elves', '{G}', 1.0, 'Creature — Elf Druid', 'Creature',
                'Elf, Druid', NULL, '{T}: Add {G}.', 'G', 'G', NULL, '1', '1', NULL, 'M19',
                '183', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'promo-only', 'Command Tower', NULL, 0.0, 'Land', 'Land', NULL, NULL,
                'Add one mana of any color in your Commander''s color identity.',
                NULL, NULL, NULL, NULL, NULL, NULL, 'PPRO', '1', 'paper', 1, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'no-scryfall', 'Obscure Test Card', NULL, 0.0, 'Land', 'Land', NULL, NULL,
                NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'NST', '1', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'oversized-only', 'Oversized Test Card', NULL, 0.0, 'Land', 'Land', NULL, NULL,
                NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'OSIZ', '1', 'paper', 0, 1, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'sylvan-5ed', 'Sylvan Library', '{G}', 1.0, 'Enchantment', 'Enchantment', NULL,
                NULL, 'At the beginning of your draw step, draw two additional cards.',
                'G', 'G', NULL, NULL, NULL, NULL, '5ED', '1', 'paper', 0, 0, 0, 'normal'
            );
            INSERT INTO cards VALUES (
                'sylvan-ptc', 'Sylvan Library', '{G}', 1.0, 'Enchantment', 'Enchantment', NULL,
                NULL, 'At the beginning of your draw step, draw two additional cards.',
                'G', 'G', NULL, NULL, NULL, NULL, 'PTC', '1', 'paper', 1, 0, 0, 'normal'
            );

            INSERT INTO cardLegalities VALUES ('sol-lea', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('sol-c21', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('atraxa', 'Legal', '');
            INSERT INTO cardLegalities VALUES ('llanowar', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('sylvan-5ed', 'Legal', '');
            INSERT INTO cardLegalities VALUES ('sylvan-ptc', NULL, NULL);

            INSERT INTO cardRulings VALUES ('atraxa', '2023-02-04', 'Proliferate ruling one.');
            INSERT INTO cardRulings VALUES ('atraxa', '2023-02-04', 'Proliferate ruling two.');

            INSERT INTO cardIdentifiers VALUES ('sol-lea', 'scryfall-sol-lea');
            INSERT INTO cardIdentifiers VALUES ('sol-c21', 'scryfall-sol-c21');
            INSERT INTO cardIdentifiers VALUES ('promo-only', 'scryfall-promo-only');
            INSERT INTO cardIdentifiers VALUES ('oversized-only', 'scryfall-oversized-only');

            INSERT INTO sets VALUES ('LEA', 'Limited Edition Alpha', '1993-08-05', 'core', 'Core Set', 295, 295);
            INSERT INTO sets VALUES ('C21', 'Commander 2021', '2021-04-23', 'commander', NULL, 81, 81);
            INSERT INTO sets VALUES ('PPRO', 'Promo Pack', '2020-01-01', 'promo', NULL, NULL, NULL);
            INSERT INTO sets VALUES ('NST', 'No Scryfall Test', '2020-01-01', 'promo', NULL, NULL, NULL);
            INSERT INTO sets VALUES ('OSIZ', 'Oversized Test', '2020-01-01', 'promo', NULL, NULL, NULL);
            "#,
        )
        .unwrap();
        (dir, path)
    }

    /// Étend `fixture_db` avec des Cartes à deux Faces (issue #56) : un
    /// layout par famille (`transform`, `modal_dfc`, `split`), isolées dans
    /// leur propre fixture pour ne pas perturber les filtres de recherche
    /// testés sur `fixture_db` (noms/mana values choisis pour ne recouper
    /// aucun filtre existant).
    fn fixture_db_with_multiface() -> (tempfile::TempDir, std::path::PathBuf) {
        let (dir, path) = fixture_db();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO cards VALUES (
                'delver-transform', 'Delver of Secrets // Insectile Aberration', NULL, 1.0,
                'Creature — Human Wizard', 'Creature', 'Human, Wizard', NULL,
                'At the beginning of your upkeep, look at the top card of your library.',
                'U', 'U', NULL, '1', '1', NULL, 'ISD', '51', 'paper', 0, 0, 0, 'transform'
            );
            INSERT INTO cards VALUES (
                'valki-mdfc', 'Valki, God of Lies // Tibalt, Cosmic Impostor', NULL, 30.0,
                'Legendary Creature — God', 'Creature', 'God', 'Legendary',
                'If a permanent entering the battlefield causes a triggered ability...',
                'B, R', 'B', NULL, '3', '3', NULL, 'KHM', '91', 'paper', 0, 0, 0, 'modal_dfc'
            );
            INSERT INTO cards VALUES (
                'fire-split', 'Fire // Ice', NULL, 30.0, 'Instant', 'Instant', NULL, NULL,
                'Fire deals 2 damage divided as you choose among one or two targets.',
                'R, U', 'R, U', NULL, NULL, NULL, NULL, 'GPT', '119', 'paper', 0, 0, 0, 'split'
            );

            INSERT INTO cardIdentifiers VALUES ('delver-transform', 'scryfall-delver');
            INSERT INTO cardIdentifiers VALUES ('valki-mdfc', 'scryfall-valki');
            INSERT INTO cardIdentifiers VALUES ('fire-split', 'scryfall-fire-ice');

            INSERT INTO sets VALUES ('ISD', 'Innistrad', '2011-09-30', 'expansion', NULL, 264, 264);
            INSERT INTO sets VALUES ('KHM', 'Kaldheim', '2021-02-05', 'expansion', NULL, 285, 285);
            INSERT INTO sets VALUES ('GPT', 'Guildpact', '2006-05-01', 'expansion', NULL, 165, 165);
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
    fn is_legal_commander_true_for_legal_card() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert_eq!(
            db.is_legal_commander("Atraxa, Praetors' Voice").unwrap(),
            Some(true)
        );
    }

    #[test]
    fn is_legal_commander_none_for_unknown_card() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert_eq!(db.is_legal_commander("Nope").unwrap(), None);
    }

    /// Régression #35 : Sylvan Library a une Impression papier légale
    /// (5ED) et une Impression promo (PTC) sans légalité renseignée
    /// (`commander` NULL). La Carte doit rester légale dès qu'une seule
    /// Impression l'est, peu importe l'ordre de retour des lignes SQL.
    #[test]
    fn is_legal_commander_true_when_any_printing_is_legal() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert_eq!(db.is_legal_commander("Sylvan Library").unwrap(), Some(true));
    }

    #[test]
    fn missing_db_file_errors_with_update_hint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let err = CardsDb::open(&path).unwrap_err();
        assert!(err.to_string().contains("kb update"));
    }

    #[test]
    fn search_dedupes_printings_by_name() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                name: Some("Sol Ring".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_filters_by_partial_name() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                name: Some("elve".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Llanowar Elves");
    }

    #[test]
    fn search_filters_by_subtype() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                subtype_contains: Some("Elf".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Llanowar Elves");
    }

    #[test]
    fn search_filters_by_legality() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                legal_in_format: Some("standard".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Sol Ring"));
        assert!(names.contains(&"Llanowar Elves"));
        assert!(!names.contains(&"Atraxa, Praetors' Voice"));
    }

    #[test]
    fn search_filters_by_mana_value_range() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                mana_value_min: Some(2.0),
                mana_value_max: Some(5.0),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Atraxa, Praetors' Voice"]);
    }

    #[test]
    fn search_with_repeated_text_combines_as_and() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        // "Add" matche Sol Ring et Llanowar Elves, mais "{C}" ne matche que
        // Sol Ring : la combinaison en ET des deux doit isoler Sol Ring.
        let results = db
            .search(&SearchFilters {
                oracle_text_contains: vec!["Add".to_string(), "{C}".to_string()],
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Sol Ring"]);
    }

    #[test]
    fn search_with_zero_limit_returns_the_whole_matching_pool() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                legal_in_format: Some("commander".to_string()),
                limit: 0,
                ..Default::default()
            })
            .unwrap();
        // Toutes les Cartes légales Commander de la fixture, sans plafond ni
        // troncature : Sol Ring (dédoublonné), Atraxa, Llanowar Elves,
        // Sylvan Library (dédoublonnée, légale via son Impression 5ED).
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Sol Ring"));
        assert!(names.contains(&"Atraxa, Praetors' Voice"));
        assert!(names.contains(&"Llanowar Elves"));
        assert!(names.contains(&"Sylvan Library"));
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn search_rejects_unknown_format() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let err = db
            .search(&SearchFilters {
                legal_in_format: Some("not-a-format".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap_err();
        assert!(err.to_string().contains("format inconnu"));
    }

    #[test]
    fn search_filters_by_color_identity_subset() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let results = db
            .search(&SearchFilters {
                color_identity_subset_of: Some(vec!["G".to_string()]),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let names: Vec<_> = results.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Sol Ring"));
        assert!(names.contains(&"Llanowar Elves"));
        assert!(!names.contains(&"Atraxa, Praetors' Voice"));
    }

    #[test]
    fn set_by_code_returns_info() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let set = db.set_by_code("lea").unwrap().expect("set found");
        assert_eq!(set.code, "LEA");
        assert_eq!(set.name, "Limited Edition Alpha");
    }

    #[test]
    fn rulings_by_name_orders_by_date() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let rulings = db
            .rulings_by_name("Atraxa, Praetors' Voice")
            .unwrap()
            .expect("card found");
        assert_eq!(rulings.len(), 2);
        assert_eq!(rulings[0].date, "2023-02-04");
    }

    #[test]
    fn rulings_by_name_none_for_unknown_card() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert!(db.rulings_by_name("Nope").unwrap().is_none());
    }

    #[test]
    fn reference_printing_picks_the_most_recent_non_promo_paper_printing() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Sol Ring")
            .unwrap()
            .expect("printing found");
        assert_eq!(printing.set_code, "C21");
        assert_eq!(printing.number, "263");
        assert_eq!(printing.scryfall_id, "scryfall-sol-c21");
        assert!(!printing.is_two_faced);
    }

    #[test]
    fn reference_printing_flags_transform_layout_as_two_faced() {
        let (_dir, path) = fixture_db_with_multiface();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Delver of Secrets // Insectile Aberration")
            .unwrap()
            .expect("printing found");
        assert!(printing.is_two_faced);
    }

    #[test]
    fn reference_printing_flags_modal_dfc_layout_as_two_faced() {
        let (_dir, path) = fixture_db_with_multiface();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Valki, God of Lies // Tibalt, Cosmic Impostor")
            .unwrap()
            .expect("printing found");
        assert!(printing.is_two_faced);
    }

    #[test]
    fn reference_printing_does_not_flag_split_layout_as_two_faced() {
        let (_dir, path) = fixture_db_with_multiface();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Fire // Ice")
            .unwrap()
            .expect("printing found");
        assert!(!printing.is_two_faced);
    }

    #[test]
    fn reference_printing_falls_back_to_promo_when_only_promo_exists() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Command Tower")
            .unwrap()
            .expect("printing found");
        assert_eq!(printing.set_code, "PPRO");
        assert_eq!(printing.scryfall_id, "scryfall-promo-only");
    }

    #[test]
    fn reference_printing_falls_back_to_oversized_when_that_is_all_there_is() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        let printing = db
            .reference_printing("Oversized Test Card")
            .unwrap()
            .expect("printing found");
        assert_eq!(printing.set_code, "OSIZ");
        assert_eq!(printing.scryfall_id, "scryfall-oversized-only");
    }

    #[test]
    fn reference_printing_none_without_any_scryfall_id() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert!(
            db.reference_printing("Obscure Test Card")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn reference_printing_none_for_unknown_card() {
        let (_dir, path) = fixture_db();
        let db = CardsDb::open(&path).unwrap();
        assert!(db.reference_printing("Not A Real Card").unwrap().is_none());
    }
}
