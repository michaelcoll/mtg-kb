use crate::db::cards::CardsDb;
use crate::model::EnrichedAnalysis;

/// Vérifie chaque Suggestion contre le glossaire (existence, légalité
/// Commander, Identité de couleur du Commandant, absence du Deck) et
/// retourne un message par violation trouvée. Ne s'arrête pas à la première
/// violation : toutes sont rapportées.
pub fn validate_suggestions(enriched: &EnrichedAnalysis, cards_db: &CardsDb) -> Vec<String> {
    let analysis = &enriched.analysis;
    let commander_identity = &analysis.commander.color_identity;
    let deck_card_names: std::collections::HashSet<&str> = analysis
        .cards
        .iter()
        .map(|c| c.card.name.as_str())
        .chain(std::iter::once(analysis.commander.name.as_str()))
        .collect();

    let mut violations = Vec::new();
    let push = |violations: &mut Vec<String>, name: &str, rule: &str| {
        violations.push(format!("Suggestion « {name} » : {rule}"));
    };

    for suggestion in &enriched.suggestions {
        let name = suggestion.card_name.as_str();

        let card = match cards_db.card_by_name(name) {
            Ok(Some(card)) => card,
            Ok(None) => {
                push(&mut violations, name, "Carte inconnue.");
                continue;
            }
            Err(e) => {
                push(
                    &mut violations,
                    name,
                    &format!("erreur lors de la résolution de la Carte ({e})."),
                );
                continue;
            }
        };

        match cards_db.is_legal_commander(name) {
            Ok(Some(true)) => {}
            Ok(_) => push(&mut violations, name, "Carte non légale en Commander."),
            Err(e) => push(
                &mut violations,
                name,
                &format!("erreur lors de la vérification de légalité ({e})."),
            ),
        }

        if !card
            .color_identity
            .iter()
            .all(|color| commander_identity.contains(color))
        {
            push(
                &mut violations,
                name,
                "hors de l'Identité de couleur du Commandant.",
            );
        }

        if deck_card_names.contains(name) {
            push(&mut violations, name, "Carte déjà présente dans le Deck.");
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT,
                setCode TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT, standard TEXT);

            INSERT INTO cards VALUES (
                'llanowar', 'Llanowar Elves', '{G}', 1.0, 'Creature — Elf Druid', 'Creature',
                'Elf, Druid', NULL, '{T}: Add {G}.', 'G', 'G', NULL, '1', '1', NULL, 'M19'
            );
            INSERT INTO cards VALUES (
                'rampant-growth', 'Rampant Growth', '{1}{G}', 2.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'Search your library for a basic land card.', 'G', 'G', NULL,
                NULL, NULL, NULL, 'M19'
            );
            INSERT INTO cards VALUES (
                'lightning-bolt', 'Lightning Bolt', '{R}', 1.0, 'Instant', 'Instant',
                NULL, NULL, 'Lightning Bolt deals 3 damage to any target.', 'R', 'R', NULL,
                NULL, NULL, NULL, '2ED'
            );
            INSERT INTO cards VALUES (
                'channel', 'Channel', '{G}', 1.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'Banned in Commander.', 'G', 'G', NULL, NULL, NULL, NULL, '2ED'
            );
            INSERT INTO cards VALUES (
                'atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature — Phyrexian Angel Horror', 'Creature',
                'Phyrexian, Angel, Horror', 'Legendary', 'Flying, vigilance...',
                'B, G, U, W', 'W, U, B, G', NULL, '4', '4', NULL, 'M15'
            );

            INSERT INTO cardLegalities VALUES ('llanowar', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('rampant-growth', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('lightning-bolt', 'Legal', 'Legal');
            INSERT INTO cardLegalities VALUES ('channel', 'Banned', 'Legal');
            INSERT INTO cardLegalities VALUES ('atraxa', 'Legal', '');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    fn sample(suggestions: Vec<Suggestion>) -> EnrichedAnalysis {
        EnrichedAnalysis {
            analysis: AnalyzeResult {
                commander: Card {
                    name: "Atraxa, Praetors' Voice".to_string(),
                    mana_cost: Some("{G}{W}{U}{B}".to_string()),
                    mana_value: Some(4.0),
                    type_line: None,
                    types: vec!["Creature".to_string()],
                    subtypes: vec![],
                    supertypes: vec![],
                    oracle_text: None,
                    color_identity: vec![
                        "B".to_string(),
                        "G".to_string(),
                        "U".to_string(),
                        "W".to_string(),
                    ],
                    colors: vec![],
                    keywords: vec![],
                    power: None,
                    toughness: None,
                    loyalty: None,
                },
                cards: vec![ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: Card {
                        name: "Llanowar Elves".to_string(),
                        mana_cost: Some("{G}".to_string()),
                        mana_value: Some(1.0),
                        type_line: None,
                        types: vec![],
                        subtypes: vec![],
                        supertypes: vec![],
                        oracle_text: None,
                        color_identity: vec!["G".to_string()],
                        colors: vec![],
                        keywords: vec![],
                        power: None,
                        toughness: None,
                        loyalty: None,
                    },
                }],
                unresolved: vec![],
                card_count: 100,
                construction_errors: vec![],
                mana_curve: ManaCurve {
                    buckets: vec![],
                    average_mana_value: 2.5,
                },
                mana_base: ManaBase {
                    land_count: 37,
                    sources_by_color: BTreeMap::new(),
                    symbols_by_color: BTreeMap::new(),
                },
                role_counts: BTreeMap::new(),
                weaknesses: vec![],
                synergies: vec![],
                candidates: vec![],
            },
            verdict: Verdict {
                summary: "Solide".to_string(),
                strengths: vec![],
                weaknesses: vec![],
                priorities: vec![],
            },
            suggestions,
        }
    }

    #[test]
    fn no_violation_for_a_legal_in_identity_absent_suggestion() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Rampant Growth".to_string(),
            justification: "Comble le manque de ramp".to_string(),
        }]);
        assert!(validate_suggestions(&enriched, &db).is_empty());
    }

    #[test]
    fn unknown_card_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Not A Real Card".to_string(),
            justification: "N'existe pas".to_string(),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Not A Real Card"));
        assert!(violations[0].contains("inconnue"));
    }

    #[test]
    fn illegal_in_commander_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Channel".to_string(),
            justification: "Trop puissante".to_string(),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Channel"));
        assert!(violations[0].contains("légale"));
    }

    #[test]
    fn outside_color_identity_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Lightning Bolt".to_string(),
            justification: "Removal".to_string(),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Lightning Bolt"));
        assert!(violations[0].contains("Identité de couleur"));
    }

    #[test]
    fn already_in_deck_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Llanowar Elves".to_string(),
            justification: "Ramp".to_string(),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Llanowar Elves"));
        assert!(violations[0].contains("déjà présente"));
    }

    #[test]
    fn suggesting_the_commander_itself_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Atraxa, Praetors' Voice".to_string(),
            justification: "?".to_string(),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("déjà présente"));
    }

    #[test]
    fn all_violations_are_reported_not_just_the_first() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![
            Suggestion {
                card_name: "Not A Real Card".to_string(),
                justification: "N'existe pas".to_string(),
            },
            Suggestion {
                card_name: "Lightning Bolt".to_string(),
                justification: "Removal".to_string(),
            },
        ]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 2);
    }
}
