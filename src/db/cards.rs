use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, Row};

use crate::model::{Card, Face, Layout, ReferencePrinting, Ruling, SetInfo, split_csv_field};

/// Whitelist : le nom de colonne est injecté tel quel dans le SQL.
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
    /// Combinées en ET.
    pub oracle_text_contains: Vec<String>,
    pub color_identity_subset_of: Option<Vec<String>>,
    pub legal_in_format: Option<String>,
    pub mana_value: Option<f64>,
    pub mana_value_min: Option<f64>,
    pub mana_value_max: Option<f64>,
    /// `0` = sans limite.
    pub limit: usize,
}

fn printing_legal_in(column: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM cardLegalities cl WHERE cl.uuid = c.uuid AND cl.{column} = 'Legal')"
    )
}

/// Une ligne par Face et par Impression ; `rows_to_cards` les regroupe.
fn card_columns() -> String {
    format!(
        "c.name, c.side, c.layout, c.faceName, c.manaCost, c.manaValue, c.faceManaValue, \
         c.type, c.types, c.subtypes, c.supertypes, c.text, c.colorIdentity, c.colors, \
         c.keywords, c.power, c.toughness, c.loyalty, {} AS legal",
        printing_legal_in("commander")
    )
}

struct CardRow {
    name: String,
    side: Option<String>,
    layout: Layout,
    mana_value: Option<f64>,
    color_identity: Vec<String>,
    legal_in_commander: bool,
    face: Face,
}

fn card_row(row: &Row) -> rusqlite::Result<CardRow> {
    let csv = |i: usize| -> rusqlite::Result<Vec<String>> {
        Ok(split_csv_field(row.get::<_, Option<String>>(i)?.as_deref()))
    };
    let name: String = row.get(0)?;
    let mana_value: Option<f64> = row.get(5)?;
    let face_name: Option<String> = row.get(3)?;
    let face_mana_value: Option<f64> = row.get(6)?;
    Ok(CardRow {
        side: row.get(1)?,
        layout: Layout::from_mtgjson(row.get::<_, Option<String>>(2)?.as_deref()),
        mana_value,
        color_identity: csv(12)?,
        legal_in_commander: row.get(18)?,
        face: Face {
            name: face_name.unwrap_or_else(|| name.clone()),
            mana_cost: row.get(4)?,
            mana_value: face_mana_value.or(mana_value),
            type_line: row.get(7)?,
            types: csv(8)?,
            subtypes: csv(9)?,
            supertypes: csv(10)?,
            oracle_text: row.get(11)?,
            colors: csv(13)?,
            keywords: csv(14)?,
            power: row.get(15)?,
            toughness: row.get(16)?,
            loyalty: row.get(17)?,
        },
        name,
    })
}

/// `rows` doit être trié par nom puis par `side` : les lignes d'une même
/// Carte (Faces × Impressions) sont consécutives, la Face principale d'abord.
fn rows_to_cards(rows: impl IntoIterator<Item = CardRow>) -> Vec<Card> {
    let mut cards: Vec<Card> = Vec::new();
    for row in rows {
        match cards.last_mut() {
            Some(card) if card.name == row.name => {
                card.legal_in_commander |= row.legal_in_commander;
                if card.back.is_none()
                    && card.layout.is_multi_face()
                    && row.side.as_deref() == Some("b")
                {
                    card.back = Some(row.face);
                }
            }
            _ => cards.push(Card {
                name: row.name,
                mana_value: row.mana_value,
                color_identity: row.color_identity,
                layout: row.layout,
                legal_in_commander: row.legal_in_commander,
                front: row.face,
                back: None,
            }),
        }
    }
    cards
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

    /// Par nom complet (`A // B` pour une Carte multi-face) ou par nom de la
    /// Face principale seule.
    pub fn card(&self, name: &str) -> Result<Option<Card>> {
        let Some(name) = self.canonical_name(name)? else {
            return Ok(None);
        };
        let sql = format!(
            "SELECT {} FROM cards c WHERE c.name = ?1 ORDER BY c.side, c.uuid",
            card_columns()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([name], card_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows_to_cards(rows).into_iter().next())
    }

    /// Cartes légales en Commander dans l'Identité de couleur donnée.
    pub fn commander_pool(&self, color_identity: &[String]) -> Result<Vec<Card>> {
        self.search(&SearchFilters {
            legal_in_format: Some("commander".to_string()),
            color_identity_subset_of: Some(color_identity.to_vec()),
            limit: 0,
            ..Default::default()
        })
    }

    /// Une Carte correspond dès qu'une de ses lignes (Face ou Impression)
    /// correspond ; le filtre d'Identité de couleur est appliqué en Rust.
    pub fn search(&self, filters: &SearchFilters) -> Result<Vec<Card>> {
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
            conditions.push(printing_legal_in(column));
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

        let mut names_sql = "SELECT DISTINCT c.name FROM cards c".to_string();
        if !conditions.is_empty() {
            names_sql.push_str(" WHERE ");
            names_sql.push_str(&conditions.join(" AND "));
        }
        names_sql.push_str(" ORDER BY c.name");
        // Sur-échantillonne : le filtre d'Identité de couleur vient après.
        if filters.limit > 0 {
            let fetch_cap = filters.limit.saturating_mul(20).max(500);
            names_sql.push_str(&format!(" LIMIT {fetch_cap}"));
        }
        let sql = format!(
            "SELECT DISTINCT {} FROM cards c WHERE c.name IN ({names_sql}) \
             ORDER BY c.name, c.side, c.uuid",
            card_columns()
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt
            .query_map(param_refs.as_slice(), card_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut results: Vec<Card> = rows_to_cards(rows)
            .into_iter()
            .filter(|card| {
                filters
                    .color_identity_subset_of
                    .as_ref()
                    .is_none_or(|allowed| {
                        card.color_identity
                            .iter()
                            .all(|c| allowed.iter().any(|a| a.eq_ignore_ascii_case(c)))
                    })
            })
            .collect();
        if filters.limit > 0 {
            results.truncate(filters.limit);
        }
        Ok(results)
    }

    /// Nom complet de la Carte désignée par `name` : son nom complet, ou le
    /// nom de sa Face principale.
    fn canonical_name(&self, name: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM cards WHERE name = ?1 OR (faceName = ?1 AND side = 'a') \
             ORDER BY name = ?1 DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([name])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
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

    pub fn rulings_by_name(&self, name: &str) -> Result<Option<Vec<Ruling>>> {
        let Some(name) = self.canonical_name(name)? else {
            return Ok(None);
        };
        let mut uuid_stmt = self
            .conn
            .prepare("SELECT uuid FROM cards WHERE name = ?1 ORDER BY side LIMIT 1")?;
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

    /// La plus récente en papier, hors promo/surdimensionnée/fantaisie ; ces
    /// filtres sont relâchés successivement à défaut.
    pub fn reference_printing(&self, name: &str) -> Result<Option<ReferencePrinting>> {
        let Some(name) = self.canonical_name(name)? else {
            return Ok(None);
        };
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
                 ORDER BY s.releaseDate DESC, c.side LIMIT 1"
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query([&name])?;
            if let Some(row) = rows.next()? {
                let layout: Option<String> = row.get(3)?;
                return Ok(Some(ReferencePrinting {
                    set_code: row.get(0)?,
                    number: row.get(1)?,
                    scryfall_id: row.get(2)?,
                    is_two_faced: Layout::from_mtgjson(layout.as_deref()).has_back_image(),
                }));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    fn base_fixture() -> CardsFixture {
        let sylvan = |uuid| {
            FixtureCard::new(uuid, "Sylvan Library")
                .mana("{G}", 1.0)
                .types("Enchantment")
                .text("At the beginning of your draw step, draw two additional cards.")
                .identity("G")
        };
        CardsFixture::new()
            .cards([
                FixtureCard::new("sol-lea", "Sol Ring")
                    .mana("{1}", 1.0)
                    .types("Artifact")
                    .text("{T}: Add {C}{C}.")
                    .printing("LEA", "1")
                    .standard("Legal"),
                FixtureCard::new("sol-c21", "Sol Ring")
                    .mana("{1}", 1.0)
                    .types("Artifact")
                    .text("{T}: Add {C}{C}.")
                    .printing("C21", "263")
                    .standard("Legal"),
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .mana("{G}{W}{U}{B}", 4.0)
                    .type_line("Legendary Creature — Phyrexian Angel Horror")
                    .types("Creature")
                    .subtypes("Phyrexian, Angel, Horror")
                    .supertypes("Legendary")
                    .identity("B, G, U, W")
                    .keywords("Deathtouch, Flying, Lifelink, Vigilance, Proliferate")
                    .printing("M15", "1"),
                FixtureCard::new("llanowar", "Llanowar Elves")
                    .mana("{G}", 1.0)
                    .types("Creature")
                    .subtypes("Elf, Druid")
                    .text("{T}: Add {G}.")
                    .identity("G")
                    .printing("M19", "183")
                    .standard("Legal"),
                FixtureCard::new("promo-only", "Command Tower")
                    .types("Land")
                    .printing("PPRO", "1")
                    .promo()
                    .without_commander_legality(),
                FixtureCard::new("no-scryfall", "Obscure Test Card")
                    .types("Land")
                    .printing("NST", "1")
                    .without_commander_legality(),
                FixtureCard::new("oversized-only", "Oversized Test Card")
                    .types("Land")
                    .printing("OSIZ", "1")
                    .oversized()
                    .without_commander_legality(),
                sylvan("sylvan-5ed").printing("5ED", "1"),
                sylvan("sylvan-ptc")
                    .printing("PTC", "1")
                    .promo()
                    .without_commander_legality(),
            ])
            .sql(
                r#"
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
    }

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        base_fixture().build()
    }

    /// Deux Faces (`a` puis `b`) d'une Carte multi-face, dans l'ordre inverse
    /// d'insertion pour vérifier que l'assemblage ne dépend pas de l'ordre
    /// SQLite.
    fn two_faces(
        uuid: &str,
        name: &str,
        layout: &str,
        front: (&str, &str),
        back: (&str, &str),
    ) -> [FixtureCard; 2] {
        let (front_name, front_types) = front;
        let (back_name, back_types) = back;
        [
            FixtureCard::new(&format!("{uuid}-b"), name)
                .face(layout, "b", back_name, 0.0)
                .mana_value(3.0)
                .type_line(back_types)
                .types(back_types),
            FixtureCard::new(&format!("{uuid}-a"), name)
                .face(layout, "a", front_name, 3.0)
                .mana_value(3.0)
                .type_line(front_types)
                .types(front_types),
        ]
    }

    fn fixture_db_with_multiface() -> (tempfile::TempDir, CardsDb) {
        base_fixture()
            .cards(two_faces(
                "delver",
                "Delver of Secrets // Insectile Aberration",
                "transform",
                ("Delver of Secrets", "Creature"),
                ("Insectile Aberration", "Creature"),
            ))
            .cards(two_faces(
                "bala-ged",
                "Bala Ged Recovery // Bala Ged Sanctuary",
                "modal_dfc",
                ("Bala Ged Recovery", "Sorcery"),
                ("Bala Ged Sanctuary", "Land"),
            ))
            .cards(two_faces(
                "fire-ice",
                "Fire // Ice",
                "split",
                ("Fire", "Instant"),
                ("Ice", "Instant"),
            ))
            .cards(two_faces(
                "brightcap",
                "Brightcap Badger // Fungus Frolic",
                "adventure",
                ("Brightcap Badger", "Creature"),
                ("Fungus Frolic", "Instant"),
            ))
            .cards(two_faces(
                "cut-ribbons",
                "Cut // Ribbons",
                "aftermath",
                ("Cut", "Sorcery"),
                ("Ribbons", "Sorcery"),
            ))
            .cards(two_faces(
                "budoka",
                "Budoka Gardener // Dokai, Weaver of Life",
                "flip",
                ("Budoka Gardener", "Creature"),
                ("Dokai, Weaver of Life", "Creature"),
            ))
            .card(
                FixtureCard::new(
                    "bruna",
                    "Bruna, the Fading Light // Brisela, Voice of Nightmares",
                )
                .face("meld", "a", "Bruna, the Fading Light", 7.0)
                .types("Creature"),
            )
            .sql(
                r#"
            INSERT INTO cardIdentifiers VALUES ('delver-a', 'scryfall-delver');
            INSERT INTO cardIdentifiers VALUES ('bala-ged-a', 'scryfall-bala-ged');
            INSERT INTO cardIdentifiers VALUES ('fire-ice-a', 'scryfall-fire-ice');
            UPDATE cards SET setCode = 'ISD', number = '51' WHERE uuid LIKE 'delver-%';
            UPDATE cards SET setCode = 'ZNR', number = '180' WHERE uuid LIKE 'bala-ged-%';
            UPDATE cards SET setCode = 'GPT', number = '119' WHERE uuid LIKE 'fire-ice-%';
            "#,
            )
            .build()
    }

    #[test]
    fn resolves_card_by_exact_name_deduped_across_printings() {
        let (_dir, db) = fixture_db();
        let card = db.card("Sol Ring").unwrap().expect("card found");
        assert_eq!(card.name, "Sol Ring");
        assert_eq!(card.mana_value, Some(1.0));
        assert!(card.back.is_none());
    }

    #[test]
    fn splits_multi_valued_fields() {
        let (_dir, db) = fixture_db();
        let card = db
            .card("Atraxa, Praetors' Voice")
            .unwrap()
            .expect("card found");
        assert_eq!(card.color_identity, vec!["B", "G", "U", "W"]);
        assert_eq!(card.front.subtypes, vec!["Phyrexian", "Angel", "Horror"]);
        assert_eq!(card.front.keywords.len(), 5);
    }

    #[test]
    fn unknown_card_returns_none() {
        let (_dir, db) = fixture_db();
        assert!(db.card("Not A Real Card").unwrap().is_none());
    }

    #[test]
    fn card_is_legal_in_commander_when_its_printing_is() {
        let (_dir, db) = fixture_db();
        let card = db.card("Atraxa, Praetors' Voice").unwrap().unwrap();
        assert!(card.legal_in_commander);
    }

    #[test]
    fn card_without_any_legal_printing_is_not_legal_in_commander() {
        let (_dir, db) = fixture_db();
        let card = db.card("Command Tower").unwrap().unwrap();
        assert!(!card.legal_in_commander);
    }

    #[test]
    fn a_promo_printing_with_null_legality_does_not_hide_a_legal_one() {
        let (_dir, db) = fixture_db();
        let card = db.card("Sylvan Library").unwrap().unwrap();
        assert!(card.legal_in_commander);
    }

    #[test]
    fn two_face_rows_make_one_card_with_front_then_back() {
        let (_dir, db) = fixture_db_with_multiface();
        let card = db
            .card("Bala Ged Recovery // Bala Ged Sanctuary")
            .unwrap()
            .unwrap();
        let faces: Vec<&str> = card.faces().map(|f| f.name.as_str()).collect();
        assert_eq!(faces, vec!["Bala Ged Recovery", "Bala Ged Sanctuary"]);
    }

    #[test]
    fn a_multi_face_card_keeps_the_card_mana_value_and_each_face_its_own() {
        let (_dir, db) = fixture_db_with_multiface();
        let card = db.card("Bala Ged Recovery").unwrap().unwrap();
        assert_eq!(card.mana_value, Some(3.0));
        assert_eq!(card.back.unwrap().mana_value, Some(0.0));
    }

    #[test]
    fn every_multi_face_layout_yields_two_faces() {
        let (_dir, db) = fixture_db_with_multiface();
        for (name, layout) in [
            ("Delver of Secrets", Layout::Transform),
            ("Bala Ged Recovery", Layout::ModalDfc),
            ("Fire", Layout::Split),
            ("Brightcap Badger", Layout::Adventure),
            ("Cut", Layout::Aftermath),
            ("Budoka Gardener", Layout::Flip),
        ] {
            let card = db.card(name).unwrap().expect(name);
            assert_eq!(card.layout, layout, "{name}");
            assert!(card.back.is_some(), "{name}");
        }
    }

    #[test]
    fn a_meld_card_has_a_single_face() {
        let (_dir, db) = fixture_db_with_multiface();
        let card = db.card("Bruna, the Fading Light").unwrap().unwrap();
        assert_eq!(card.layout, Layout::Meld);
        assert!(card.back.is_none());
    }

    #[test]
    fn the_secondary_face_name_alone_does_not_resolve() {
        let (_dir, db) = fixture_db_with_multiface();
        assert!(db.card("Fungus Frolic").unwrap().is_none());
    }

    #[test]
    fn search_assembles_multi_face_cards_matching_on_either_face() {
        let (_dir, db) = fixture_db_with_multiface();
        let results = db
            .search(&SearchFilters {
                type_contains: Some("Land".to_string()),
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let bala_ged = results
            .iter()
            .find(|c| c.name == "Bala Ged Recovery // Bala Ged Sanctuary")
            .expect("matched through its land face");
        assert_eq!(bala_ged.front.name, "Bala Ged Recovery");
    }

    #[test]
    fn commander_pool_keeps_legal_cards_within_the_identity() {
        let (_dir, db) = fixture_db();
        let pool = db.commander_pool(&["G".to_string()]).unwrap();
        let names: Vec<_> = pool.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Llanowar Elves", "Sol Ring", "Sylvan Library"]);
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db();
        let set = db.set_by_code("lea").unwrap().expect("set found");
        assert_eq!(set.code, "LEA");
        assert_eq!(set.name, "Limited Edition Alpha");
    }

    #[test]
    fn rulings_by_name_orders_by_date() {
        let (_dir, db) = fixture_db();
        let rulings = db
            .rulings_by_name("Atraxa, Praetors' Voice")
            .unwrap()
            .expect("card found");
        assert_eq!(rulings.len(), 2);
        assert_eq!(rulings[0].date, "2023-02-04");
    }

    #[test]
    fn rulings_by_name_none_for_unknown_card() {
        let (_dir, db) = fixture_db();
        assert!(db.rulings_by_name("Nope").unwrap().is_none());
    }

    #[test]
    fn reference_printing_picks_the_most_recent_non_promo_paper_printing() {
        let (_dir, db) = fixture_db();
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
        let (_dir, db) = fixture_db_with_multiface();
        let printing = db
            .reference_printing("Delver of Secrets // Insectile Aberration")
            .unwrap()
            .expect("printing found");
        assert!(printing.is_two_faced);
    }

    #[test]
    fn reference_printing_flags_modal_dfc_layout_as_two_faced() {
        let (_dir, db) = fixture_db_with_multiface();
        let printing = db
            .reference_printing("Bala Ged Recovery // Bala Ged Sanctuary")
            .unwrap()
            .expect("printing found");
        assert!(printing.is_two_faced);
    }

    #[test]
    fn reference_printing_does_not_flag_split_layout_as_two_faced() {
        let (_dir, db) = fixture_db_with_multiface();
        let printing = db
            .reference_printing("Fire // Ice")
            .unwrap()
            .expect("printing found");
        assert!(!printing.is_two_faced);
    }

    #[test]
    fn reference_printing_falls_back_to_promo_when_only_promo_exists() {
        let (_dir, db) = fixture_db();
        let printing = db
            .reference_printing("Command Tower")
            .unwrap()
            .expect("printing found");
        assert_eq!(printing.set_code, "PPRO");
        assert_eq!(printing.scryfall_id, "scryfall-promo-only");
    }

    #[test]
    fn reference_printing_falls_back_to_oversized_when_that_is_all_there_is() {
        let (_dir, db) = fixture_db();
        let printing = db
            .reference_printing("Oversized Test Card")
            .unwrap()
            .expect("printing found");
        assert_eq!(printing.set_code, "OSIZ");
        assert_eq!(printing.scryfall_id, "scryfall-oversized-only");
    }

    #[test]
    fn reference_printing_none_without_any_scryfall_id() {
        let (_dir, db) = fixture_db();
        assert!(
            db.reference_printing("Obscure Test Card")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn reference_printing_none_for_unknown_card() {
        let (_dir, db) = fixture_db();
        assert!(db.reference_printing("Not A Real Card").unwrap().is_none());
    }
}
