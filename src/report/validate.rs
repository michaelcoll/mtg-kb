use crate::db::cards::CardsDb;
use crate::model::EnrichedAnalysis;

/// Un message par violation (existence, légalité Commander, Identité de
/// couleur, absence du Deck).
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

        let card = match cards_db.card(name) {
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

        if !card.legal_in_commander {
            push(&mut violations, name, "Carte non légale en Commander.");
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

        if deck_card_names.contains(card.name.as_str()) {
            push(&mut violations, name, "Carte déjà présente dans le Deck.");
        }

        if let Some(card_to_remove) = &suggestion.card_to_remove
            && !deck_card_names.contains(card_to_remove.as_str())
        {
            push(
                &mut violations,
                name,
                &format!("Carte à retirer « {card_to_remove} » absente du Deck."),
            );
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::model::*;
    use std::collections::BTreeMap;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("llanowar", "Llanowar Elves").identity("G"),
                FixtureCard::new("rampant-growth", "Rampant Growth").identity("G"),
                FixtureCard::new("lightning-bolt", "Lightning Bolt").identity("R"),
                FixtureCard::new("channel", "Channel")
                    .identity("G")
                    .banned(),
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice").identity("B, G, U, W"),
            ])
            .build()
    }

    fn sample(suggestions: Vec<Suggestion>) -> EnrichedAnalysis {
        EnrichedAnalysis {
            analysis: AnalyzeResult {
                commander: Card::named("Atraxa, Praetors' Voice", &["B", "G", "U", "W"]),
                cards: vec![ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: Card::named("Llanowar Elves", &["G"]),
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
                edhrec_recommendations: vec![],
                edhrec_unresolved_names: vec![],
                recommander_recommendations: vec![],
                recommander_unresolved_names: vec![],
                source_errors: vec![],
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
            card_to_remove: None,
        }]);
        assert!(validate_suggestions(&enriched, &db).is_empty());
    }

    #[test]
    fn no_violation_when_card_to_remove_is_in_the_deck() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Rampant Growth".to_string(),
            justification: "Comble le manque de ramp".to_string(),
            card_to_remove: Some("Llanowar Elves".to_string()),
        }]);
        assert!(validate_suggestions(&enriched, &db).is_empty());
    }

    #[test]
    fn card_to_remove_absent_from_deck_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Rampant Growth".to_string(),
            justification: "Comble le manque de ramp".to_string(),
            card_to_remove: Some("Sol Ring".to_string()),
        }]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Sol Ring"));
        assert!(violations[0].contains("absente du Deck"));
    }

    #[test]
    fn unknown_card_is_a_violation() {
        let (_dir, db) = fixture_db();
        let enriched = sample(vec![Suggestion {
            card_name: "Not A Real Card".to_string(),
            justification: "N'existe pas".to_string(),
            card_to_remove: None,
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
            card_to_remove: None,
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
            card_to_remove: None,
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
            card_to_remove: None,
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
            card_to_remove: None,
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
                card_to_remove: None,
            },
            Suggestion {
                card_name: "Lightning Bolt".to_string(),
                justification: "Removal".to_string(),
                card_to_remove: None,
            },
        ]);
        let violations = validate_suggestions(&enriched, &db);
        assert_eq!(violations.len(), 2);
    }
}
