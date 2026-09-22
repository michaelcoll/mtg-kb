pub mod candidates;
pub mod external;
pub mod metrics;
pub mod ranking;
pub mod themes;

use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, bail};

use crate::db::cards::CardsDb;
use crate::decklist::parser::{self, DecklistLine};
use crate::model::{AnalyzeResult, ResolvedCard, Synergy, UnresolvedLine};

const REQUIRED_DECK_SIZE: u32 = 100;

const MAJOR_THEME_MIN_CARDS: u32 = 8;

/// Erreur uniquement si le Commandant est ambigu (0 ou 2+) : Cartes non
/// résolues et écarts de validation sont reportés dans le résultat.
pub fn run(input: &str, db: &CardsDb, thresholds: &metrics::Thresholds) -> Result<AnalyzeResult> {
    let decklist = parser::parse(input);

    if decklist.commander.is_empty() {
        bail!("aucun Commandant trouvé dans la section Commander");
    }
    let aggregated_commander = aggregate(decklist.commander);
    if aggregated_commander.len() != 1 || aggregated_commander[0].quantity != 1 {
        bail!(
            "un Deck Commander doit avoir exactement un Commandant (trouvé : {})",
            aggregated_commander
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let commander_name = &aggregated_commander[0].name;
    let Some(commander) = db.card_by_name(commander_name)? else {
        bail!("Commandant introuvable dans la Base cartes : « {commander_name} »");
    };

    let mut cards = Vec::new();
    let mut unresolved = Vec::new();
    let mut construction_errors = Vec::new();
    let mut weaknesses = Vec::new();

    for line in aggregate(decklist.deck) {
        match db.card_by_name(&line.name)? {
            Some(card) => cards.push(ResolvedCard {
                quantity: line.quantity,
                roles: metrics::detect_roles(&card),
                themes: themes::detect_themes(&card),
                card,
            }),
            None => unresolved.push(UnresolvedLine {
                quantity: line.quantity,
                name: line.name,
            }),
        }
    }

    let card_count: u32 = 1
        + cards.iter().map(|c| c.quantity).sum::<u32>()
        + unresolved.iter().map(|u| u.quantity).sum::<u32>();
    if card_count != REQUIRED_DECK_SIZE {
        construction_errors.push(format!(
            "le Deck contient {card_count} cartes, {REQUIRED_DECK_SIZE} attendues"
        ));
    }

    for resolved in &cards {
        if resolved.quantity > 1 && !is_basic_land(&resolved.card) {
            construction_errors.push(format!(
                "« {} » apparaît {} fois : le Deck doit être singleton hors terrains de base",
                resolved.card.name, resolved.quantity
            ));
        }
        if !is_color_identity_subset(&resolved.card.color_identity, &commander.color_identity) {
            construction_errors.push(format!(
                "« {} » (Identité de couleur {:?}) dépasse l'Identité de couleur du Commandant {:?}",
                resolved.card.name, resolved.card.color_identity, commander.color_identity
            ));
        }
        match db.is_legal_commander(&resolved.card.name)? {
            Some(false) | None => weaknesses.push(format!(
                "« {} » n'est pas légale en Commander",
                resolved.card.name
            )),
            Some(true) => {}
        }
    }

    let mana_curve = metrics::mana_curve(&cards);
    let mana_base = metrics::mana_base(&cards, &commander);
    let mut role_counts: BTreeMap<String, u32> = BTreeMap::new();
    for resolved in &cards {
        for role in &resolved.roles {
            *role_counts.entry(role.clone()).or_insert(0) += resolved.quantity;
        }
    }
    weaknesses.extend(metrics::role_weaknesses(
        &role_counts,
        mana_base.land_count,
        thresholds,
    ));
    weaknesses.extend(metrics::curve_weaknesses(&mana_curve, thresholds));

    let synergies = find_synergies(&cards);

    let deck_names: HashSet<String> = cards.iter().map(|c| c.card.name.clone()).collect();
    let mut theme_counts: BTreeMap<String, u32> = BTreeMap::new();
    for resolved in &cards {
        for theme in &resolved.themes {
            *theme_counts.entry(theme.clone()).or_insert(0) += resolved.quantity;
        }
    }
    // Les Thèmes du Commandant ne comptent pas dans le seuil de Thème majeur.
    let major_themes: HashSet<String> = theme_counts
        .into_iter()
        .filter(|(_, count)| *count >= MAJOR_THEME_MIN_CARDS)
        .map(|(theme, _)| theme)
        .collect();
    let weak_roles = metrics::weak_role_names(&role_counts, thresholds);
    let candidates = candidates::find_candidates(
        db,
        &commander.color_identity,
        &deck_names,
        &major_themes,
        &weak_roles,
    )?;

    Ok(AnalyzeResult {
        commander,
        cards,
        unresolved,
        card_count,
        construction_errors,
        mana_curve,
        mana_base,
        role_counts,
        weaknesses,
        synergies,
        candidates,
        // Remplies ensuite par `commands::analyze`, hors `--offline`.
        edhrec_recommendations: Vec::new(),
        edhrec_unresolved_names: Vec::new(),
        recommander_recommendations: Vec::new(),
        recommander_unresolved_names: Vec::new(),
        source_errors: Vec::new(),
    })
}

fn find_synergies(cards: &[ResolvedCard]) -> Vec<Synergy> {
    let mut cards_by_theme: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for resolved in cards {
        for theme in &resolved.themes {
            cards_by_theme
                .entry(theme.clone())
                .or_default()
                .push(resolved.card.name.clone());
        }
    }
    cards_by_theme
        .into_iter()
        .filter(|(_, cards)| cards.len() >= 2)
        .map(|(theme, cards)| Synergy { theme, cards })
        .collect()
}

fn aggregate(lines: Vec<DecklistLine>) -> Vec<DecklistLine> {
    let mut merged: Vec<DecklistLine> = Vec::new();
    for line in lines {
        if let Some(existing) = merged
            .iter_mut()
            .find(|l: &&mut DecklistLine| l.name == line.name)
        {
            existing.quantity += line.quantity;
        } else {
            merged.push(line);
        }
    }
    merged
}

pub(crate) fn is_basic_land(card: &crate::model::Card) -> bool {
    card.supertypes.iter().any(|t| t == "Basic") && card.types.iter().any(|t| t == "Land")
}

fn is_color_identity_subset(card_identity: &[String], commander_identity: &[String]) -> bool {
    card_identity
        .iter()
        .all(|c| commander_identity.iter().any(|a| a.eq_ignore_ascii_case(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                faceName TEXT, side TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature', 'Phyrexian, Angel, Horror',
                'Legendary', 'text', 'B, G, U, W', 'W, U, B, G', NULL, '4', '4', NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('solring', 'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact',
                NULL, NULL, '{T}: Add {C}{C}.', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('forest', 'Forest', NULL, 0.0, 'Basic Land — Forest', 'Land',
                'Forest', 'Basic', 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('lotus', 'Black Lotus', '{0}', 0.0, 'Artifact', 'Artifact',
                NULL, NULL, 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('shock', 'Shock', '{R}', 1.0, 'Instant', 'Instant',
                NULL, NULL, 'text', 'R', 'R', NULL, NULL, NULL, NULL);

            INSERT INTO cardLegalities VALUES ('atraxa', 'Legal');
            INSERT INTO cardLegalities VALUES ('solring', 'Legal');
            INSERT INTO cardLegalities VALUES ('forest', 'Legal');
            INSERT INTO cardLegalities VALUES ('lotus', 'Banned');
            INSERT INTO cardLegalities VALUES ('shock', 'Legal');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    fn deck_of(n: u32, extra: &str) -> String {
        let mut lines = String::from("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n");
        lines.push_str(&format!("{n} Forest\n"));
        lines.push_str(extra);
        lines
    }

    #[test]
    fn errors_when_no_commander() {
        let (_dir, db) = fixture_db();
        let err = run("Deck\n1 Sol Ring\n", &db, &metrics::Thresholds::default()).unwrap_err();
        assert!(err.to_string().contains("aucun Commandant"));
    }

    #[test]
    fn errors_when_two_commanders() {
        let (_dir, db) = fixture_db();
        let input = "Commander\n1 Atraxa, Praetors' Voice\n1 Sol Ring\n\nDeck\n";
        let err = run(input, &db, &metrics::Thresholds::default()).unwrap_err();
        assert!(err.to_string().contains("exactement un Commandant"));
    }

    #[test]
    fn reports_unresolved_cards_without_failing() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Some Unknown Card\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.unresolved[0].name, "Some Unknown Card");
    }

    #[test]
    fn flags_wrong_deck_size() {
        let (_dir, db) = fixture_db();
        let input = deck_of(10, "");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("100 attendues"))
        );
    }

    #[test]
    fn allows_many_basic_lands_but_not_duplicate_nonland() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "2 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            !result
                .construction_errors
                .iter()
                .any(|p| p.contains("Forest"))
        );
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("Sol Ring"))
        );
    }

    #[test]
    fn flags_color_identity_violation() {
        let (_dir, db) = fixture_db();
        // Atraxa is WUBG; Shock is red-identity, out of Commander's colors.
        let input = deck_of(97, "1 Shock\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("Shock") && p.contains("Identité de couleur"))
        );
    }

    #[test]
    fn flags_illegal_card_as_weakness() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "1 Black Lotus\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .weaknesses
                .iter()
                .any(|p| p.contains("Black Lotus") && p.contains("légale"))
        );
    }

    #[test]
    fn valid_100_card_deck_has_no_construction_errors() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(result.card_count, 100);
        assert!(
            result.construction_errors.is_empty(),
            "{:?}",
            result.construction_errors
        );
    }

    #[test]
    fn mostly_lands_deck_reports_ramp_and_draw_weaknesses() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Ramp sous-représenté"))
        );
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Pioche sous-représenté"))
        );
        assert_eq!(result.mana_base.land_count, 98);
        assert_eq!(result.role_counts.get("terrain"), Some(&98));
        assert_eq!(result.role_counts.get("ramp"), Some(&1));
    }

    fn fixture_db_with_goblin_pool(goblins_in_deck: u32) -> (tempfile::TempDir, CardsDb, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                faceName TEXT, side TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature', 'Phyrexian, Angel, Horror',
                'Legendary', 'text', 'B, G, U, W', 'W, U, B, G', NULL, '4', '4', NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('forest', 'Forest', NULL, 0.0, 'Basic Land — Forest', 'Land',
                'Forest', 'Basic', 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('goblin-pool', 'Goblin Raider', '{1}{G}', 2.0, 'Creature',
                'Creature', 'Goblin', NULL, '', 'G', 'G', NULL, '2', '2', NULL);

            INSERT INTO cardLegalities VALUES ('atraxa', 'Legal');
            INSERT INTO cardLegalities VALUES ('forest', 'Legal');
            INSERT INTO cardLegalities VALUES ('goblin-pool', 'Legal');
            "#,
        )
        .unwrap();

        let mut deck_lines = String::new();
        for i in 0..goblins_in_deck {
            let uuid = format!("goblin-deck-{i}");
            let name = format!("Goblin Grunt {i}");
            conn.execute(
                "INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, \
                 supertypes, text, colorIdentity, colors, keywords, power, toughness, loyalty) \
                 VALUES (?1, ?2, '{G}', 1.0, 'Creature', 'Creature', 'Goblin', \
                 NULL, '', 'G', 'G', NULL, '1', '1', NULL)",
                rusqlite::params![uuid, name],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cardLegalities VALUES (?1, 'Legal')",
                rusqlite::params![uuid],
            )
            .unwrap();
            deck_lines.push_str(&format!("1 {name}\n"));
        }

        (dir, CardsDb::open(&path).unwrap(), deck_lines)
    }

    #[test]
    fn a_minor_tribal_theme_yields_no_candidate() {
        let (_dir, db, goblin_lines) = fixture_db_with_goblin_pool(3);
        let input =
            format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n95 Forest\n{goblin_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Goblin Raider"),
            "tribal:Goblin is carried by only 3 cards, below the major-theme threshold"
        );
    }

    #[test]
    fn a_theme_carried_by_at_least_eight_cards_is_major_and_yields_candidates() {
        let (_dir, db, goblin_lines) = fixture_db_with_goblin_pool(8);
        let input =
            format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n90 Forest\n{goblin_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        let raider = result
            .candidates
            .iter()
            .find(|c| c.card.name == "Goblin Raider")
            .expect("tribal:Goblin, carried by 8 deck cards, is a major theme");
        assert_eq!(raider.matched_themes, vec!["tribal:Goblin".to_string()]);
    }

    #[test]
    fn the_commander_does_not_count_towards_the_major_theme_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                faceName TEXT, side TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('goblin-commander', 'Goblin Warlord', '{2}{R}{R}', 4.0,
                'Legendary Creature — Goblin Warrior', 'Creature', 'Goblin, Warrior',
                'Legendary', 'text', 'R', 'R', NULL, '4', '4', NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('forest', 'Forest', NULL, 0.0, 'Basic Land — Forest', 'Land',
                'Forest', 'Basic', 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, supertypes,
                text, colorIdentity, colors, keywords, power, toughness, loyalty)
                VALUES ('goblin-pool', 'Goblin Raider', '{1}{R}', 2.0, 'Creature',
                'Creature', 'Goblin', NULL, '', 'R', 'R', NULL, '2', '2', NULL);

            INSERT INTO cardLegalities VALUES ('goblin-commander', 'Legal');
            INSERT INTO cardLegalities VALUES ('forest', 'Legal');
            INSERT INTO cardLegalities VALUES ('goblin-pool', 'Legal');
            "#,
        )
        .unwrap();

        // 7 Gobelins + le Commandant Gobelin : 8 si le Commandant était compté.
        let mut deck_lines = String::new();
        for i in 0..7 {
            let uuid = format!("goblin-deck-{i}");
            let name = format!("Goblin Grunt {i}");
            conn.execute(
                "INSERT INTO cards (uuid, name, manaCost, manaValue, type, types, subtypes, \
                 supertypes, text, colorIdentity, colors, keywords, power, toughness, loyalty) \
                 VALUES (?1, ?2, '{R}', 1.0, 'Creature', 'Creature', 'Goblin', \
                 NULL, '', 'R', 'R', NULL, '1', '1', NULL)",
                rusqlite::params![uuid, name],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cardLegalities VALUES (?1, 'Legal')",
                rusqlite::params![uuid],
            )
            .unwrap();
            deck_lines.push_str(&format!("1 {name}\n"));
        }

        let db = CardsDb::open(&path).unwrap();
        let input = format!("Commander\n1 Goblin Warlord\n\nDeck\n92 Forest\n{deck_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Goblin Raider"),
            "tribal:Goblin is carried by only 7 Deck cards; the Commander must not count towards \
             the 8-card major-theme threshold"
        );
    }
}
