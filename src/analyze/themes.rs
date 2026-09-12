use std::sync::LazyLock;

use regex::Regex;

use crate::model::Card;

/// Thèmes détectés par motifs sur le texte oracle et les sous-types de
/// Créature. Une Carte peut porter 0..n Thèmes. Le Thème "tribal:<sous-type>"
/// n'est pas une liste figée de tribus : n'importe quel sous-type de
/// Créature devient un Thème, et c'est le partage de ce Thème par plusieurs
/// Cartes du Deck (voir Synergies) qui en fait une tribu pertinente.
pub fn detect_themes(card: &Card) -> Vec<String> {
    static TOKENS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)creates? .*tokens?").unwrap());
    static PLUS_ONE_COUNTERS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\+1/\+1 counters?").unwrap());
    static ARISTOCRATS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(whenever .* dies|sacrifice (a|an|another) creature)").unwrap()
    });

    let mut themes = Vec::new();
    let text = card.oracle_text.as_deref().unwrap_or("");

    if TOKENS.is_match(text) {
        themes.push("tokens".to_string());
    }
    if PLUS_ONE_COUNTERS.is_match(text) {
        themes.push("+1/+1".to_string());
    }
    if ARISTOCRATS.is_match(text) {
        themes.push("aristocrats".to_string());
    }
    if card.types.iter().any(|t| t == "Creature") {
        for subtype in &card.subtypes {
            themes.push(format!("tribal:{subtype}"));
        }
    }
    themes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, oracle_text: &str, types: &[&str], subtypes: &[&str]) -> Card {
        Card {
            name: name.to_string(),
            mana_cost: None,
            mana_value: None,
            type_line: None,
            types: types.iter().map(|t| t.to_string()).collect(),
            subtypes: subtypes.iter().map(|t| t.to_string()).collect(),
            supertypes: vec![],
            oracle_text: Some(oracle_text.to_string()),
            color_identity: vec![],
            colors: vec![],
            keywords: vec![],
            power: None,
            toughness: None,
            loyalty: None,
        }
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
