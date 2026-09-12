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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    pub card_name: String,
    pub justification: String,
}

/// Le JSON de `kb analyze` enrichi par Claude : verdict et Suggestions
/// retenues parmi les candidats. `kb report` prend ce JSON en entrée.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrichedAnalysis {
    #[serde(flatten)]
    pub analysis: AnalyzeResult,
    pub verdict: String,
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
}

pub fn split_csv_field(raw: Option<&str>) -> Vec<String> {
    match raw {
        Some(s) if !s.is_empty() => s.split(", ").map(|p| p.trim().to_string()).collect(),
        _ => Vec::new(),
    }
}
