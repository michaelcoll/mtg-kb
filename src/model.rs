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

/// L'Impression de référence d'une Carte (voir CONTEXT.md) : celle utilisée
/// pour illustrer la Carte dans le Rapport d'analyse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferencePrinting {
    pub scryfall_id: String,
    pub set_code: String,
    pub number: String,
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

/// Une Recommandation externe d'EDHREC (voir CONTEXT.md) : `synergy` et le
/// taux d'inclusion (`num_decks / potential_decks`) tels que renvoyés par
/// EDHREC, et le `header` de la liste d'origine (ex. "High Synergy Cards").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdhrecRecommendation {
    pub card: Card,
    pub synergy: f64,
    pub inclusion_rate: f64,
    pub header: String,
}

/// Une Recommandation externe de Recommander (voir CONTEXT.md).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecommanderRecommendation {
    pub card: Card,
    pub score: f64,
}

/// L'échec d'une Source externe (réseau, HTTP 429, structure JSON
/// inattendue) : n'interrompt pas `kb analyze`, porté ici pour signalement
/// dans le Rapport d'analyse (voir ADR 0003).
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
    /// Écarts de construction du Deck (taille, singleton, Identité de
    /// couleur) : des erreurs de deckbuilding, pas des Points faibles
    /// stratégiques.
    pub construction_errors: Vec<String>,
    pub mana_curve: ManaCurve,
    pub mana_base: ManaBase,
    pub role_counts: BTreeMap<String, u32>,
    /// Points faibles au sens du glossaire : Rôle sous-représenté, courbe de
    /// mana déséquilibrée, base de mana insuffisante, Carte illégale.
    pub weaknesses: Vec<String>,
    pub synergies: Vec<Synergy>,
    /// Cartes légales Commander, dans l'Identité de couleur, absentes du
    /// Deck, classées par Thèmes du Deck et Rôles en Point faible — base des
    /// Suggestions choisies par Claude.
    pub candidates: Vec<Candidate>,
    /// Recommandations externes d'EDHREC (voir ADR 0003), déjà filtrées
    /// (résolues, légales en Commander, dans l'Identité de couleur, absentes
    /// du Deck), triées par synergie décroissante, au plus 30. Vide en mode
    /// `--offline` ou si la Source a échoué (voir `source_errors`).
    #[serde(default)]
    pub edhrec_recommendations: Vec<EdhrecRecommendation>,
    /// Noms renvoyés par EDHREC qui ne correspondent à aucune Carte de la
    /// Base cartes.
    #[serde(default)]
    pub edhrec_unresolved_names: Vec<String>,
    /// Recommandations externes de Recommander (voir ADR 0003), mêmes
    /// filtres, triées par score décroissant, au plus 30.
    #[serde(default)]
    pub recommander_recommendations: Vec<RecommanderRecommendation>,
    /// Noms renvoyés par Recommander qui ne correspondent à aucune Carte de
    /// la Base cartes.
    #[serde(default)]
    pub recommander_unresolved_names: Vec<String>,
    /// Échecs de Source externe (voir ADR 0003) : ne bloquent pas l'analyse.
    #[serde(default)]
    pub source_errors: Vec<SourceError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    pub card_name: String,
    pub justification: String,
    /// Carte à retirer (voir CONTEXT.md) : facultative, une Carte du Deck
    /// que cette Suggestion propose de remplacer.
    #[serde(default)]
    pub card_to_remove: Option<String>,
}

/// L'appréciation d'ensemble d'un Deck (voir CONTEXT.md) : Résumé, Points
/// forts, Faiblesses (qualitatives, distinctes des Points faibles mesurés
/// par `kb`) et Priorités (actions d'amélioration, ordonnées par
/// importance).
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

/// Le JSON de `kb analyze` enrichi par Claude : verdict et Suggestions
/// retenues parmi les candidats. `kb report` prend ce JSON en entrée.
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

    /// Analyse minimale valide, en `serde_json::Value` pour que les tests
    /// puissent y fusionner un `verdict` sans manipuler du JSON en chaîne.
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
