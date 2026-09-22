//! Base cartes de test : un seul schéma, des Cartes décrites par
//! `FixtureCard` plutôt que par des `INSERT` positionnels.

use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::cards::CardsDb;

const SCHEMA: &str = r#"
    CREATE TABLE cards (
        uuid TEXT, name TEXT, faceName TEXT, side TEXT, layout TEXT DEFAULT 'normal',
        manaCost TEXT, manaValue REAL, faceManaValue REAL, type TEXT, types TEXT,
        subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT, colors TEXT,
        keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
        setCode TEXT, number TEXT, availability TEXT DEFAULT 'paper',
        isPromo BOOLEAN DEFAULT 0, isOversized BOOLEAN DEFAULT 0, isFunny BOOLEAN DEFAULT 0
    );
    CREATE TABLE cardLegalities (uuid TEXT, commander TEXT, standard TEXT);
    CREATE TABLE cardRulings (uuid TEXT, date TEXT, text TEXT);
    CREATE TABLE cardIdentifiers (uuid TEXT, scryfallId TEXT);
    CREATE TABLE sets (
        code TEXT, name TEXT, releaseDate TEXT, type TEXT, block TEXT,
        baseSetSize INTEGER, totalSetSize INTEGER
    );
"#;

/// Une ligne de `cards` (une Face d'une Impression) et sa légalité.
#[derive(Debug, Clone)]
pub struct FixtureCard {
    uuid: String,
    name: String,
    face_name: Option<String>,
    side: Option<String>,
    layout: String,
    mana_cost: Option<String>,
    mana_value: f64,
    face_mana_value: Option<f64>,
    type_line: Option<String>,
    types: Option<String>,
    subtypes: Option<String>,
    supertypes: Option<String>,
    text: Option<String>,
    color_identity: Option<String>,
    keywords: Option<String>,
    set_code: Option<String>,
    number: Option<String>,
    is_promo: bool,
    is_oversized: bool,
    commander: Option<String>,
    standard: Option<String>,
}

impl FixtureCard {
    /// Légale en Commander par défaut.
    pub fn new(uuid: &str, name: &str) -> Self {
        Self {
            uuid: uuid.to_string(),
            name: name.to_string(),
            face_name: None,
            side: None,
            layout: "normal".to_string(),
            mana_cost: None,
            mana_value: 0.0,
            face_mana_value: None,
            type_line: None,
            types: None,
            subtypes: None,
            supertypes: None,
            text: None,
            color_identity: None,
            keywords: None,
            set_code: None,
            number: None,
            is_promo: false,
            is_oversized: false,
            commander: Some("Legal".to_string()),
            standard: None,
        }
    }

    pub fn mana(mut self, cost: &str, value: f64) -> Self {
        self.mana_cost = Some(cost.to_string());
        self.mana_value = value;
        self
    }

    pub fn mana_value(mut self, value: f64) -> Self {
        self.mana_value = value;
        self
    }

    /// `types` au format MTGJSON ("Artifact, Creature").
    pub fn types(mut self, types: &str) -> Self {
        self.types = Some(types.to_string());
        self
    }

    pub fn type_line(mut self, type_line: &str) -> Self {
        self.type_line = Some(type_line.to_string());
        self
    }

    pub fn subtypes(mut self, subtypes: &str) -> Self {
        self.subtypes = Some(subtypes.to_string());
        self
    }

    pub fn supertypes(mut self, supertypes: &str) -> Self {
        self.supertypes = Some(supertypes.to_string());
        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }

    pub fn identity(mut self, color_identity: &str) -> Self {
        self.color_identity = Some(color_identity.to_string());
        self
    }

    pub fn keywords(mut self, keywords: &str) -> Self {
        self.keywords = Some(keywords.to_string());
        self
    }

    /// Face `side` ("a", "b") d'une Carte multi-face de layout `layout`.
    pub fn face(mut self, layout: &str, side: &str, face_name: &str, face_mana_value: f64) -> Self {
        self.layout = layout.to_string();
        self.side = Some(side.to_string());
        self.face_name = Some(face_name.to_string());
        self.face_mana_value = Some(face_mana_value);
        self
    }

    pub fn printing(mut self, set_code: &str, number: &str) -> Self {
        self.set_code = Some(set_code.to_string());
        self.number = Some(number.to_string());
        self
    }

    pub fn promo(mut self) -> Self {
        self.is_promo = true;
        self
    }

    pub fn oversized(mut self) -> Self {
        self.is_oversized = true;
        self
    }

    /// `cardLegalities.commander` à NULL, comme certaines promos.
    pub fn without_commander_legality(mut self) -> Self {
        self.commander = None;
        self
    }

    pub fn banned(mut self) -> Self {
        self.commander = Some("Banned".to_string());
        self
    }

    pub fn standard(mut self, legality: &str) -> Self {
        self.standard = Some(legality.to_string());
        self
    }
}

pub struct CardsFixture {
    dir: TempDir,
    conn: Connection,
}

impl CardsFixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("AllPrintings.sqlite")).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Self { dir, conn }
    }

    pub fn card(self, card: FixtureCard) -> Self {
        self.conn
            .execute(
                "INSERT INTO cards (uuid, name, faceName, side, layout, manaCost, manaValue, \
                 faceManaValue, type, types, subtypes, supertypes, text, colorIdentity, \
                 keywords, setCode, number, isPromo, isOversized) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                 ?17, ?18, ?19)",
                params![
                    card.uuid,
                    card.name,
                    card.face_name,
                    card.side,
                    card.layout,
                    card.mana_cost,
                    card.mana_value,
                    card.face_mana_value,
                    card.type_line,
                    card.types,
                    card.subtypes,
                    card.supertypes,
                    card.text,
                    card.color_identity,
                    card.keywords,
                    card.set_code,
                    card.number,
                    card.is_promo,
                    card.is_oversized,
                ],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO cardLegalities (uuid, commander, standard) VALUES (?1, ?2, ?3)",
                params![card.uuid, card.commander, card.standard],
            )
            .unwrap();
        self
    }

    pub fn cards(self, cards: impl IntoIterator<Item = FixtureCard>) -> Self {
        cards.into_iter().fold(self, Self::card)
    }

    /// Pour les tables annexes (sets, rulings, identifiants, Impressions).
    pub fn sql(self, sql: &str) -> Self {
        self.conn.execute_batch(sql).unwrap();
        self
    }

    pub fn build(self) -> (TempDir, CardsDb) {
        let path = self.dir.path().join("AllPrintings.sqlite");
        drop(self.conn);
        let db = CardsDb::open(&path).unwrap();
        (self.dir, db)
    }
}
