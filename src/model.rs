use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Layout MTGJSON d'une Carte. Seuls les layouts qui changent la
/// modélisation sont distingués ; les autres sont regroupés dans `Other`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Normal,
    Transform,
    ModalDfc,
    Split,
    Adventure,
    Aftermath,
    Flip,
    Meld,
    #[serde(other)]
    Other,
}

impl Layout {
    pub fn from_mtgjson(layout: Option<&str>) -> Self {
        use serde::de::IntoDeserializer;
        use serde::de::value::{Error, StrDeserializer};
        layout.map_or(Self::Normal, |layout| {
            let deserializer: StrDeserializer<Error> = layout.into_deserializer();
            Self::deserialize(deserializer).unwrap_or(Self::Other)
        })
    }

    /// Layouts où `name` combine deux Faces d'une même Carte (ADR 0004) ;
    /// `meld` en est exclu : deux Cartes physiques distinctes.
    pub fn is_multi_face(self) -> bool {
        matches!(
            self,
            Self::Transform
                | Self::ModalDfc
                | Self::Split
                | Self::Adventure
                | Self::Aftermath
                | Self::Flip
        )
    }

    /// Carte physique recto verso : l'image de la Face arrière est distincte.
    pub fn has_back_image(self) -> bool {
        matches!(self, Self::Transform | Self::ModalDfc)
    }
}

/// Une moitié jouable d'une Carte ; une Carte normale n'a qu'une Face.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Face {
    pub name: String,
    pub mana_cost: Option<String>,
    pub mana_value: Option<f64>,
    pub type_line: Option<String>,
    pub types: Vec<String>,
    pub subtypes: Vec<String>,
    pub supertypes: Vec<String>,
    pub oracle_text: Option<String>,
    pub colors: Vec<String>,
    pub keywords: Vec<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
}

impl Face {
    pub fn is_land(&self) -> bool {
        self.types.iter().any(|t| t == "Land")
    }
}

/// Une Carte et ses Faces (ADR 0004). `mana_value` est celle de la Carte
/// (celle qui compte hors de la pile), `front.mana_value` celle de la Face.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(from = "CardJson", into = "CardJson")]
pub struct Card {
    pub name: String,
    pub mana_value: Option<f64>,
    pub color_identity: Vec<String>,
    pub layout: Layout,
    /// Au moins une Impression légale en Commander.
    pub legal_in_commander: bool,
    pub front: Face,
    pub back: Option<Face>,
}

impl Card {
    pub fn faces(&self) -> impl Iterator<Item = &Face> {
        std::iter::once(&self.front).chain(self.back.as_ref())
    }

    /// Terrain dès qu'une Face l'est (Carte modale sort // terrain).
    pub fn is_land(&self) -> bool {
        self.faces().any(Face::is_land)
    }

    pub fn is_basic_land(&self) -> bool {
        self.front.supertypes.iter().any(|t| t == "Basic") && self.front.is_land()
    }

    /// Union des étiquettes (Rôles, Thèmes) détectées sur chaque Face
    /// (ADR 0004), dans l'ordre de première apparition.
    pub fn union_over_faces(&self, detect: impl Fn(&Face) -> Vec<String>) -> Vec<String> {
        let mut union: Vec<String> = Vec::new();
        for label in self.faces().flat_map(detect) {
            if !union.contains(&label) {
                union.push(label);
            }
        }
        union
    }

    /// Carte de test à une Face, légale, dans l'Identité de couleur donnée.
    #[cfg(test)]
    pub fn named(name: &str, color_identity: &[&str]) -> Self {
        Self {
            color_identity: color_identity.iter().map(|c| c.to_string()).collect(),
            ..Self::from_face(Face {
                name: name.to_string(),
                ..Face::default()
            })
        }
    }

    #[cfg(test)]
    pub fn from_face(front: Face) -> Self {
        Self {
            name: front.name.clone(),
            mana_value: front.mana_value,
            legal_in_commander: true,
            front,
            ..Self::default()
        }
    }
}

/// Forme JSON d'une Carte (contrat avec Claude, ADR 0001) : les champs de
/// premier niveau restent ceux de la Face principale, `faces` n'apparaît que
/// pour une Carte multi-face.
#[derive(Serialize, Deserialize)]
struct CardJson {
    name: String,
    mana_cost: Option<String>,
    mana_value: Option<f64>,
    type_line: Option<String>,
    types: Vec<String>,
    subtypes: Vec<String>,
    supertypes: Vec<String>,
    oracle_text: Option<String>,
    color_identity: Vec<String>,
    colors: Vec<String>,
    keywords: Vec<String>,
    power: Option<String>,
    toughness: Option<String>,
    loyalty: Option<String>,
    #[serde(default)]
    layout: Layout,
    #[serde(default)]
    legal_in_commander: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    faces: Vec<Face>,
}

impl From<Card> for CardJson {
    fn from(card: Card) -> Self {
        let faces = match card.back {
            Some(back) => vec![card.front.clone(), back],
            None => Vec::new(),
        };
        let front = card.front;
        Self {
            name: card.name,
            mana_cost: front.mana_cost,
            mana_value: card.mana_value,
            type_line: front.type_line,
            types: front.types,
            subtypes: front.subtypes,
            supertypes: front.supertypes,
            oracle_text: front.oracle_text,
            color_identity: card.color_identity,
            colors: front.colors,
            keywords: front.keywords,
            power: front.power,
            toughness: front.toughness,
            loyalty: front.loyalty,
            layout: card.layout,
            legal_in_commander: card.legal_in_commander,
            faces,
        }
    }
}

impl From<CardJson> for Card {
    fn from(json: CardJson) -> Self {
        let mut faces = json.faces.into_iter();
        let (front, back) = match (faces.next(), faces.next()) {
            (Some(front), back) => (front, back),
            (None, _) => (
                Face {
                    name: json.name.clone(),
                    mana_cost: json.mana_cost,
                    mana_value: json.mana_value,
                    type_line: json.type_line,
                    types: json.types,
                    subtypes: json.subtypes,
                    supertypes: json.supertypes,
                    oracle_text: json.oracle_text,
                    colors: json.colors,
                    keywords: json.keywords,
                    power: json.power,
                    toughness: json.toughness,
                    loyalty: json.loyalty,
                },
                None,
            ),
        };
        Self {
            name: json.name,
            mana_value: json.mana_value,
            color_identity: json.color_identity,
            layout: json.layout,
            legal_in_commander: json.legal_in_commander,
            front,
            back,
        }
    }
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

    fn face(name: &str, types: &[&str]) -> Face {
        Face {
            name: name.to_string(),
            types: types.iter().map(|t| t.to_string()).collect(),
            ..Face::default()
        }
    }

    fn bala_ged() -> Card {
        Card {
            name: "Bala Ged Recovery // Bala Ged Sanctuary".to_string(),
            mana_value: Some(3.0),
            layout: Layout::ModalDfc,
            legal_in_commander: true,
            front: face("Bala Ged Recovery", &["Sorcery"]),
            back: Some(face("Bala Ged Sanctuary", &["Land"])),
            ..Card::default()
        }
    }

    #[test]
    fn single_face_card_json_keeps_flat_fields_without_faces() {
        let card = Card::from_face(face("Sol Ring", &["Artifact"]));
        let json = serde_json::to_value(&card).unwrap();
        assert_eq!(json["types"], serde_json::json!(["Artifact"]));
        assert!(json.get("faces").is_none());
    }

    #[test]
    fn multi_face_card_json_exposes_both_faces_with_front_as_flat_fields() {
        let json = serde_json::to_value(bala_ged()).unwrap();
        assert_eq!(json["types"], serde_json::json!(["Sorcery"]));
        assert_eq!(json["layout"], "modal_dfc");
        assert_eq!(json["faces"][1]["name"], "Bala Ged Sanctuary");
    }

    #[test]
    fn card_json_round_trips() {
        let card = bala_ged();
        let json = serde_json::to_string(&card).unwrap();
        assert_eq!(serde_json::from_str::<Card>(&json).unwrap(), card);
    }

    #[test]
    fn unknown_layout_deserializes_as_other() {
        let layout: Layout = serde_json::from_str("\"saga\"").unwrap();
        assert_eq!(layout, Layout::Other);
    }

    #[test]
    fn a_card_is_a_land_when_any_face_is() {
        assert!(bala_ged().is_land());
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
