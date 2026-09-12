use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SetInfo {
    pub code: String,
    pub name: String,
    pub release_date: Option<String>,
    pub set_type: Option<String>,
    pub block: Option<String>,
    pub base_set_size: Option<i64>,
    pub total_set_size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ruling {
    pub date: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleWithChildren {
    pub number: String,
    pub title: Option<String>,
    pub text: Option<String>,
    pub children: Vec<RuleEntryOut>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleEntryOut {
    pub number: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GlossaryDefinition {
    pub term: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnresolvedLine {
    pub quantity: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResolvedCard {
    pub quantity: u32,
    pub card: Card,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnalyzeResult {
    pub commander: Card,
    pub cards: Vec<ResolvedCard>,
    pub unresolved: Vec<UnresolvedLine>,
    pub card_count: u32,
    pub problems: Vec<String>,
}

pub fn split_csv_field(raw: Option<&str>) -> Vec<String> {
    match raw {
        Some(s) if !s.is_empty() => s.split(", ").map(|p| p.trim().to_string()).collect(),
        _ => Vec::new(),
    }
}
