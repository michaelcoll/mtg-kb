use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Card {
    pub name: String,
    pub mana_cost: Option<String>,
    pub mana_value: Option<f64>,
    pub type_line: Option<String>,
    pub types: Vec<String>,
    pub subtypes: Vec<String>,
    pub supertypes: Vec<String>,
    pub oracle_text: Option<String>,
    pub color_identity: Vec<String>,
    pub colors: Vec<String>,
    pub keywords: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetInfo {
    pub code: String,
    pub name: String,
    pub release_date: Option<String>,
    pub set_type: Option<String>,
    pub block: Option<String>,
    pub base_set_size: Option<i64>,
    pub total_set_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferencePrinting {
    pub scryfall_id: String,
    pub set_code: String,
    pub number: String,
    /// Layout `transform` ou `modal_dfc` : face arrière via `&face=back`.
    pub is_two_faced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ruling {
    pub date: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleWithChildren {
    pub number: String,
    pub title: Option<String>,
    pub text: Option<String>,
    pub children: Vec<RuleEntryOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleEntryOut {
    pub number: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlossaryDefinition {
    pub term: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnresolvedLine {
    pub quantity: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedCard {
    pub quantity: u32,
    pub roles: Vec<String>,
    pub themes: Vec<String>,
    pub card: Card,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Synergy {
    pub theme: String,
    pub cards: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub card: Card,
    pub score: u32,
    pub matched_themes: Vec<String>,
    pub matched_weak_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdhrecRecommendation {
    pub card: Card,
    pub synergy: f64,
    /// `num_decks / potential_decks`
    pub inclusion_rate: f64,
    /// Liste EDHREC d'origine (ex. "High Synergy Cards").
    pub header: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecommanderRecommendation {
    pub card: Card,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceError {
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManaCurveBucket {
    pub mana_value: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManaCurve {
    pub buckets: Vec<ManaCurveBucket>,
    pub average_mana_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManaBase {
    pub land_count: u32,
    pub sources_by_color: BTreeMap<String, u32>,
    pub symbols_by_color: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeResult {
    pub commander: Card,
    pub cards: Vec<ResolvedCard>,
    pub unresolved: Vec<UnresolvedLine>,
    pub card_count: u32,
    /// Taille, singleton, Identité de couleur.
    pub construction_errors: Vec<String>,
    pub mana_curve: ManaCurve,
    pub mana_base: ManaBase,
    pub role_counts: BTreeMap<String, u32>,
    pub weaknesses: Vec<String>,
    pub synergies: Vec<Synergy>,
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub edhrec_recommendations: Vec<EdhrecRecommendation>,
    #[serde(default)]
    pub edhrec_unresolved_names: Vec<String>,
    #[serde(default)]
    pub recommander_recommendations: Vec<RecommanderRecommendation>,
    #[serde(default)]
    pub recommander_unresolved_names: Vec<String>,
    #[serde(default)]
    pub source_errors: Vec<SourceError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    pub card_name: String,
    pub justification: String,
    #[serde(default)]
    pub card_to_remove: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Verdict {
    pub summary: String,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub priorities: Vec<String>,
}

/// Le JSON de `kb analyze` enrichi par Claude, entrée de `kb report`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichedAnalysis {
    #[serde(flatten)]
    pub analysis: AnalyzeResult,
    pub verdict: Verdict,
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
}

pub fn split_csv_field(raw: Option<&str>) -> Vec<String> {
    match raw {
        Some(s) if !s.is_empty() => s.split(", ").map(|p| p.trim().to_string()).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_analysis_json(verdict: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "commander": {
                "name": "Atraxa, Praetors' Voice", "mana_cost": null, "mana_value": null,
                "type_line": null, "types": [], "subtypes": [], "supertypes": [],
                "oracle_text": null, "color_identity": [], "colors": [], "keywords": [],
                "power": null, "toughness": null, "loyalty": null
            },
            "cards": [], "unresolved": [], "card_count": 100, "construction_errors": [],
            "mana_curve": {"buckets": [], "average_mana_value": 0.0},
            "mana_base": {"land_count": 37, "sources_by_color": {}, "symbols_by_color": {}},
            "role_counts": {}, "weaknesses": [], "synergies": [], "candidates": [],
            "verdict": verdict,
        })
    }

    #[test]
    fn rejects_the_old_string_verdict_format() {
        let json = minimal_analysis_json(serde_json::json!("Solide, manque de ramp"));
        let result: Result<EnrichedAnalysis, _> = serde_json::from_value(json);
        assert!(
            result.is_err(),
            "un verdict en chaîne de texte doit être rejeté"
        );
    }

    #[test]
    fn accepts_the_structured_verdict_format() {
        let json = minimal_analysis_json(serde_json::json!({
            "summary": "Solide", "strengths": [], "weaknesses": [], "priorities": []
        }));
        let result: Result<EnrichedAnalysis, _> = serde_json::from_value(json);
        assert!(result.is_ok(), "{:?}", result.err());
    }
}
