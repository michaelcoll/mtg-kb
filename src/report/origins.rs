use std::collections::HashSet;

use crate::model::EnrichedAnalysis;

/// Origines de chaque Suggestion (`kb`, `edhrec`, `recommander`, sinon
/// `investigation`), alignées sur `enriched.suggestions`.
pub fn compute_origins(enriched: &EnrichedAnalysis) -> Vec<Vec<String>> {
    let analysis = &enriched.analysis;
    let candidate_names: HashSet<&str> = analysis
        .candidates
        .iter()
        .map(|c| c.card.name.as_str())
        .collect();
    let edhrec_names: HashSet<&str> = analysis
        .edhrec_recommendations
        .iter()
        .map(|r| r.card.name.as_str())
        .collect();
    let recommander_names: HashSet<&str> = analysis
        .recommander_recommendations
        .iter()
        .map(|r| r.card.name.as_str())
        .collect();

    enriched
        .suggestions
        .iter()
        .map(|s| {
            let name = s.card_name.as_str();
            let mut origins = Vec::new();
            if candidate_names.contains(name) {
                origins.push("kb".to_string());
            }
            if edhrec_names.contains(name) {
                origins.push("edhrec".to_string());
            }
            if recommander_names.contains(name) {
                origins.push("recommander".to_string());
            }
            if origins.is_empty() {
                origins.push("investigation".to_string());
            }
            origins
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn card(name: &str) -> Card {
        Card::named(name, &[])
    }

    fn sample(suggestions: Vec<Suggestion>) -> EnrichedAnalysis {
        EnrichedAnalysis {
            analysis: AnalyzeResult {
                commander: card("Test Commander"),
                cards: vec![],
                unresolved: vec![],
                card_count: 100,
                construction_errors: vec![],
                mana_curve: ManaCurve {
                    buckets: vec![],
                    average_mana_value: 0.0,
                },
                mana_base: ManaBase {
                    land_count: 37,
                    sources_by_color: BTreeMap::new(),
                    symbols_by_color: BTreeMap::new(),
                },
                role_counts: BTreeMap::new(),
                weaknesses: vec![],
                synergies: vec![],
                candidates: vec![Candidate {
                    card: card("Rampant Growth"),
                    score: 2,
                    matched_themes: vec![],
                    matched_weak_roles: vec!["ramp".to_string()],
                }],
                edhrec_recommendations: vec![EdhrecRecommendation {
                    card: card("Sol Ring"),
                    synergy: 0.1,
                    inclusion_rate: 0.9,
                    header: "High Synergy Cards".to_string(),
                }],
                edhrec_unresolved_names: vec![],
                recommander_recommendations: vec![RecommanderRecommendation {
                    card: card("Cultivate"),
                    score: 3.0,
                }],
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

    fn suggestion(name: &str) -> Suggestion {
        Suggestion {
            card_name: name.to_string(),
            justification: "test".to_string(),
            card_to_remove: None,
        }
    }

    #[test]
    fn a_suggestion_from_candidates_has_origin_kb() {
        let enriched = sample(vec![suggestion("Rampant Growth")]);
        assert_eq!(compute_origins(&enriched), vec![vec!["kb".to_string()]]);
    }

    #[test]
    fn a_suggestion_from_edhrec_has_origin_edhrec() {
        let enriched = sample(vec![suggestion("Sol Ring")]);
        assert_eq!(compute_origins(&enriched), vec![vec!["edhrec".to_string()]]);
    }

    #[test]
    fn a_suggestion_from_recommander_has_origin_recommander() {
        let enriched = sample(vec![suggestion("Cultivate")]);
        assert_eq!(
            compute_origins(&enriched),
            vec![vec!["recommander".to_string()]]
        );
    }

    #[test]
    fn a_suggestion_absent_from_all_three_lists_has_origin_investigation() {
        let enriched = sample(vec![suggestion("Beast Within")]);
        assert_eq!(
            compute_origins(&enriched),
            vec![vec!["investigation".to_string()]]
        );
    }

    #[test]
    fn a_suggestion_present_in_several_lists_has_several_origins() {
        let mut enriched = sample(vec![suggestion("Rampant Growth")]);
        enriched
            .analysis
            .edhrec_recommendations
            .push(EdhrecRecommendation {
                card: card("Rampant Growth"),
                synergy: 0.2,
                inclusion_rate: 0.5,
                header: "Top Cards".to_string(),
            });
        assert_eq!(
            compute_origins(&enriched),
            vec![vec!["kb".to_string(), "edhrec".to_string()]]
        );
    }

    #[test]
    fn origins_are_aligned_positionally_with_suggestions() {
        let enriched = sample(vec![
            suggestion("Rampant Growth"),
            suggestion("Beast Within"),
        ]);
        assert_eq!(
            compute_origins(&enriched),
            vec![vec!["kb".to_string()], vec!["investigation".to_string()]]
        );
    }
}
