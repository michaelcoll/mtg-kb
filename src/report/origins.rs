use crate::model::AnalyzeResult;

/// Provenance d'une Suggestion (CONTEXT.md) : Candidat de `kb`,
/// Recommandation externe d'une Source, sinon investigation hors de ces listes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Kb,
    Edhrec,
    Recommander,
    Investigation,
}

impl Origin {
    /// Libellé du badge dans le Rapport d'analyse.
    pub fn label(self) -> &'static str {
        match self {
            Origin::Kb => "kb",
            Origin::Edhrec => "edhrec",
            Origin::Recommander => "recommander",
            Origin::Investigation => "investigation",
        }
    }
}

/// Origines de la Suggestion `card_name`, dans l'ordre `kb`, `edhrec`,
/// `recommander` ; `Investigation` seule si aucune liste ne la contient.
pub fn origins_of(analysis: &AnalyzeResult, card_name: &str) -> Vec<Origin> {
    let mut origins = Vec::new();
    if analysis.candidates.iter().any(|c| c.card.name == card_name) {
        origins.push(Origin::Kb);
    }
    if analysis
        .edhrec_recommendations
        .iter()
        .any(|r| r.card.name == card_name)
    {
        origins.push(Origin::Edhrec);
    }
    if analysis
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
        analysis.edhrec_recommendations.push(EdhrecRecommendation {
            card: card("Rampant Growth"),
            synergy: 0.2,
            inclusion_rate: 0.5,
            header: "Top Cards".to_string(),
        });
        assert_eq!(
            origins_of(&analysis, "Rampant Growth"),
            vec![Origin::Kb, Origin::Edhrec]
        );
    }
}
