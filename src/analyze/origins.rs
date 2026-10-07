use crate::model::{AnalyzeResult, Origin};

/// Origines de la Carte `card_name` parmi les Candidats et les Recommandations
/// externes, dans l'ordre `kb`, `edhrec`, `recommander` ; `Investigation` seule
/// si aucune liste ne la contient. Même règle pour les Candidats, les
/// Recommandations externes et les Suggestions du Rapport.
pub fn origins_of(analysis: &AnalyzeResult, card_name: &str) -> Vec<Origin> {
    let mut origins = Vec::new();
    if analysis.candidates.iter().any(|c| c.card.name == card_name) {
        origins.push(Origin::Kb);
    }
    if analysis
        .external
        .edhrec_recommendations
        .iter()
        .any(|r| r.card.name == card_name)
    {
        origins.push(Origin::Edhrec);
    }
    if analysis
        .external
        .recommander_recommendations
        .iter()
        .any(|r| r.card.name == card_name)
    {
        origins.push(Origin::Recommander);
    }
    if origins.is_empty() {
        origins.push(Origin::Investigation);
    }
    origins
}

/// Renseigne les Origines de chaque Candidat et de chaque Recommandation
/// externe, une fois toutes les listes connues.
pub fn assign_origins(analysis: &mut AnalyzeResult) {
    let candidates: Vec<_> = analysis
        .candidates
        .iter()
        .map(|c| origins_of(analysis, &c.card.name))
        .collect();
    let edhrec: Vec<_> = analysis
        .external
        .edhrec_recommendations
        .iter()
        .map(|r| origins_of(analysis, &r.card.name))
        .collect();
    let recommander: Vec<_> = analysis
        .external
        .recommander_recommendations
        .iter()
        .map(|r| origins_of(analysis, &r.card.name))
        .collect();

    for (c, o) in analysis.candidates.iter_mut().zip(candidates) {
        c.origins = o;
    }
    for (r, o) in analysis
        .external
        .edhrec_recommendations
        .iter_mut()
        .zip(edhrec)
    {
        r.origins = o;
    }
    for (r, o) in analysis
        .external
        .recommander_recommendations
        .iter_mut()
        .zip(recommander)
    {
        r.origins = o;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use std::collections::BTreeMap;

    fn card(name: &str) -> Card {
        Card::named(name, &[])
    }

    fn sample() -> AnalyzeResult {
        AnalyzeResult {
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
                origins: vec![],
            }],
            external: ExternalSources {
                edhrec_recommendations: vec![EdhrecRecommendation {
                    card: card("Sol Ring"),
                    synergy: 0.1,
                    inclusion_rate: 0.9,
                    header: "High Synergy Cards".to_string(),
                    origins: vec![],
                }],
                recommander_recommendations: vec![RecommanderRecommendation {
                    card: card("Cultivate"),
                    score: 3.0,
                    origins: vec![],
                }],
                ..ExternalSources::default()
            },
        }
    }

    #[test]
    fn a_suggestion_from_candidates_has_origin_kb() {
        assert_eq!(origins_of(&sample(), "Rampant Growth"), vec![Origin::Kb]);
    }

    #[test]
    fn a_suggestion_from_edhrec_has_origin_edhrec() {
        assert_eq!(origins_of(&sample(), "Sol Ring"), vec![Origin::Edhrec]);
    }

    #[test]
    fn a_suggestion_from_recommander_has_origin_recommander() {
        assert_eq!(
            origins_of(&sample(), "Cultivate"),
            vec![Origin::Recommander]
        );
    }

    #[test]
    fn a_suggestion_absent_from_all_three_lists_has_origin_investigation() {
        assert_eq!(
            origins_of(&sample(), "Beast Within"),
            vec![Origin::Investigation]
        );
    }

    #[test]
    fn a_suggestion_present_in_several_lists_has_several_origins() {
        let mut analysis = sample();
        analysis
            .external
            .edhrec_recommendations
            .push(EdhrecRecommendation {
                card: card("Rampant Growth"),
                synergy: 0.2,
                inclusion_rate: 0.5,
                header: "Top Cards".to_string(),
                origins: vec![],
            });
        assert_eq!(
            origins_of(&analysis, "Rampant Growth"),
            vec![Origin::Kb, Origin::Edhrec]
        );
    }
}
