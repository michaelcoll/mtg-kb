use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Card, Face};

use super::metrics::union_over_faces;

/// Thèmes détectés par motifs sur le texte oracle et les sous-types de
/// Créature. Tout sous-type de Créature devient un Thème "tribal:<sous-type>".
/// Union des Thèmes des Faces (ADR 0004).
pub fn detect_themes(card: &Card) -> Vec<String> {
    union_over_faces(card, detect_face_themes)
}

fn detect_face_themes(face: &Face) -> Vec<String> {
    static TOKENS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)creates? .*tokens?").unwrap());
    static PLUS_ONE_COUNTERS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\+1/\+1 counters?").unwrap());
    static ARISTOCRATS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(whenever .* dies|sacrifice (a|an|another) creature)").unwrap()
    });

    let mut themes = Vec::new();
    let text = face.oracle_text.as_deref().unwrap_or("");

    if TOKENS.is_match(text) {
        themes.push("tokens".to_string());
    }
    if PLUS_ONE_COUNTERS.is_match(text) {
        themes.push("+1/+1".to_string());
    }
    if ARISTOCRATS.is_match(text) {
        themes.push("aristocrats".to_string());
    }
    if face.types.iter().any(|t| t == "Creature") {
        for subtype in &face.subtypes {
            themes.push(format!("tribal:{subtype}"));
        }
    }
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(name: &str, oracle_text: &str, types: &[&str], subtypes: &[&str]) -> Face {
        Face {
            name: name.to_string(),
            types: types.iter().map(|t| t.to_string()).collect(),
            subtypes: subtypes.iter().map(|t| t.to_string()).collect(),
            oracle_text: Some(oracle_text.to_string()),
            ..Face::default()
        }
    }

    fn card(name: &str, oracle_text: &str, types: &[&str], subtypes: &[&str]) -> Card {
        Card::from_face(face(name, oracle_text, types, subtypes))
    }

    #[test]
    fn a_multi_face_card_has_the_union_of_its_faces_themes() {
        let c = Card {
            back: Some(face(
                "Fungus Frolic",
                "Create two 1/1 Squirrel tokens.",
                &["Instant"],
                &[],
            )),
            ..card("Brightcap Badger", "", &["Creature"], &["Badger"])
        };
        assert_eq!(
            detect_themes(&c),
            vec!["tribal:Badger".to_string(), "tokens".to_string()]
        );
    }

    #[test]
    fn detects_tokens_theme() {
        let c = card(
            "Krenko, Mob Boss",
            "Whenever Krenko attacks, create X 1/1 red Goblin creature tokens.",
            &["Creature"],
            &["Goblin"],
        );
        assert!(detect_themes(&c).contains(&"tokens".to_string()));
    }

    #[test]
    fn detects_plus_one_counters_theme() {
        let c = card(
            "Hardened Scales",
            "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.",
            &["Enchantment"],
            &[],
        );
        assert!(detect_themes(&c).contains(&"+1/+1".to_string()));
    }

    #[test]
    fn detects_aristocrats_theme() {
        let c = card(
            "Blood Artist",
            "Whenever Blood Artist or another creature dies, target player loses 1 life and you gain 1 life.",
            &["Creature"],
            &["Vampire"],
        );
        assert!(detect_themes(&c).contains(&"aristocrats".to_string()));
    }

    #[test]
    fn creature_gets_tribal_theme_per_subtype() {
        let c = card("Goblin Chieftain", "", &["Creature"], &["Goblin"]);
        assert_eq!(detect_themes(&c), vec!["tribal:Goblin".to_string()]);
    }

    #[test]
    fn noncreature_has_no_tribal_theme() {
        let c = card("Sol Ring", "{T}: Add {C}{C}.", &["Artifact"], &[]);
        assert!(detect_themes(&c).is_empty());
    }
}
