use anyhow::{Result, bail};

use crate::db::cards::CardsDb;
use crate::decklist::parser::{self, DecklistLine};
use crate::model::{AnalyzeResult, ResolvedCard, UnresolvedLine};

const REQUIRED_DECK_SIZE: u32 = 100;

/// Résout et valide une Decklist en un AnalyzeResult. Erreur explicite
/// uniquement pour l'ambiguïté de Commandant (0 ou 2+) : tout le reste
/// (Cartes non résolues, écarts de validation) est reporté dans le résultat
/// sans bloquer l'analyse, conformément à l'ADR "kb calcule, Claude juge".
pub fn run(input: &str, db: &CardsDb) -> Result<AnalyzeResult> {
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
    let mut problems = Vec::new();

    for line in aggregate(decklist.deck) {
        match db.card_by_name(&line.name)? {
            Some(card) => cards.push(ResolvedCard {
                quantity: line.quantity,
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
        problems.push(format!(
            "le Deck contient {card_count} cartes, {REQUIRED_DECK_SIZE} attendues"
        ));
    }

    for resolved in &cards {
        if resolved.quantity > 1 && !is_basic_land(&resolved.card) {
            problems.push(format!(
                "« {} » apparaît {} fois : le Deck doit être singleton hors terrains de base",
                resolved.card.name, resolved.quantity
            ));
        }
        if !is_color_identity_subset(&resolved.card.color_identity, &commander.color_identity) {
            problems.push(format!(
                "« {} » (Identité de couleur {:?}) dépasse l'Identité de couleur du Commandant {:?}",
                resolved.card.name, resolved.card.color_identity, commander.color_identity
            ));
        }
        match db.is_legal_commander(&resolved.card.name)? {
            Some(false) | None => problems.push(format!(
                "« {} » n'est pas légale en Commander",
                resolved.card.name
            )),
            Some(true) => {}
        }
    }

    Ok(AnalyzeResult {
        commander,
        cards,
        unresolved,
        card_count,
        problems,
    })
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

fn is_basic_land(card: &crate::model::Card) -> bool {
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
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards VALUES ('atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature', 'Phyrexian, Angel, Horror',
                'Legendary', 'text', 'B, G, U, W', 'W, U, B, G', NULL, '4', '4', NULL);
            INSERT INTO cards VALUES ('solring', 'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact',
                NULL, NULL, 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('forest', 'Forest', NULL, 0.0, 'Basic Land — Forest', 'Land',
                'Forest', 'Basic', 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('lotus', 'Black Lotus', '{0}', 0.0, 'Artifact', 'Artifact',
                NULL, NULL, 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('shock', 'Shock', '{R}', 1.0, 'Instant', 'Instant',
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
        let err = run("Deck\n1 Sol Ring\n", &db).unwrap_err();
        assert!(err.to_string().contains("aucun Commandant"));
    }

    #[test]
    fn errors_when_two_commanders() {
        let (_dir, db) = fixture_db();
        let input = "Commander\n1 Atraxa, Praetors' Voice\n1 Sol Ring\n\nDeck\n";
        let err = run(input, &db).unwrap_err();
        assert!(err.to_string().contains("exactement un Commandant"));
    }

    #[test]
    fn reports_unresolved_cards_without_failing() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Some Unknown Card\n");
        let result = run(&input, &db).unwrap();
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.unresolved[0].name, "Some Unknown Card");
    }

    #[test]
    fn flags_wrong_deck_size() {
        let (_dir, db) = fixture_db();
        let input = deck_of(10, "");
        let result = run(&input, &db).unwrap();
        assert!(result.problems.iter().any(|p| p.contains("100 attendues")));
    }

    #[test]
    fn allows_many_basic_lands_but_not_duplicate_nonland() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "2 Sol Ring\n");
        let result = run(&input, &db).unwrap();
        assert!(!result.problems.iter().any(|p| p.contains("Forest")));
        assert!(result.problems.iter().any(|p| p.contains("Sol Ring")));
    }

    #[test]
    fn flags_color_identity_violation() {
        let (_dir, db) = fixture_db();
        // Atraxa is WUBG; Shock is red-identity, out of Commander's colors.
        let input = deck_of(97, "1 Shock\n");
        let result = run(&input, &db).unwrap();
        assert!(
            result
                .problems
                .iter()
                .any(|p| p.contains("Shock") && p.contains("Identité de couleur"))
        );
    }

    #[test]
    fn flags_illegal_card() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "1 Black Lotus\n");
        let result = run(&input, &db).unwrap();
        assert!(
            result
                .problems
                .iter()
                .any(|p| p.contains("Black Lotus") && p.contains("légale"))
        );
    }

    #[test]
    fn valid_100_card_deck_has_no_problems() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Sol Ring\n");
        let result = run(&input, &db).unwrap();
        assert_eq!(result.card_count, 100);
        assert!(result.problems.is_empty(), "{:?}", result.problems);
    }
}
