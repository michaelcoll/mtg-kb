use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Card, Face};

use super::tribal;

/// Thèmes détectés par motifs sur le texte oracle et les sous-types de
/// Créature. Tout sous-type de Créature, et tout type de créature nommé par le
/// texte oracle, devient un Thème "tribal:<sous-type>".
/// Une Correction de Thèmes remplace la détection (ADR 0005).
pub fn detect_themes(card: &Card) -> Vec<String> {
    match &card.corrections.themes {
        Some(themes) => themes.clone(),
        None => card.union_over_faces(detect_face_themes),
    }
}

/// Poids d'un Thème tribal dont le texte oracle nomme le sous-type (lords,
/// payoffs) : il passe devant une simple créature du sous-type (poids 1).
const NAMED_SUBTYPE_WEIGHT: u32 = 2;

/// Thèmes de `detect_themes`, chacun avec son poids au score des Candidats :
/// `NAMED_SUBTYPE_WEIGHT` pour un Thème tribal dont une Face nomme le
/// sous-type, 1 sinon. Une Correction de Thèmes, qui remplace toute la
/// détection, donne le poids 1 à chacun de ses Thèmes.
pub fn detect_weighted_themes(card: &Card) -> Vec<(String, u32)> {
    let named = match &card.corrections.themes {
        Some(_) => Vec::new(),
        None => card.union_over_faces(named_tribal_themes),
    };
    detect_themes(card)
        .into_iter()
        .map(|theme| {
            let weight = if named.contains(&theme) {
                NAMED_SUBTYPE_WEIGHT
            } else {
                1
            };
            (theme, weight)
        })
        .collect()
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
    themes.extend(named_tribal_themes(face));
    themes
}

fn named_tribal_themes(face: &Face) -> Vec<String> {
    tribal::named_creature_types(face)
        .into_iter()
        .map(|t| format!("tribal:{t}"))
        .collect()
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
            vec![
                "tribal:Badger".to_string(),
                "tokens".to_string(),
                "tribal:Squirrel".to_string()
            ]
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

    #[test]
    fn a_noncreature_card_naming_a_subtype_matches_its_tribal_theme() {
        let c = card(
            "Elvish Promenade",
            "Create a 1/1 green Elf Warrior creature token for each Elf you control.",
            &["Kindred", "Sorcery"],
            &["Elf"],
        );
        assert!(detect_themes(&c).contains(&"tribal:Elf".to_string()));
    }

    #[test]
    fn a_noncreature_card_without_the_subtype_naming_it_matches_its_tribal_theme() {
        let c = card(
            "Goblin War Drums",
            "Each Goblin you control has menace.",
            &["Enchantment"],
            &[],
        );
        assert_eq!(detect_themes(&c), vec!["tribal:Goblin".to_string()]);
    }

    fn tribal_weight(c: &Card, theme: &str) -> Option<u32> {
        detect_weighted_themes(c)
            .into_iter()
            .find(|(t, _)| t == theme)
            .map(|(_, weight)| weight)
    }

    #[test]
    fn naming_the_subtype_weighs_more_than_only_having_it() {
        let magda = card(
            "Magda, Brazen Outlaw",
            "Other Dwarves you control get +1/+0.",
            &["Creature"],
            &["Dwarf", "Berserker"],
        );
        let grunt = card("Dwarf Grunt", "", &["Creature"], &["Dwarf"]);
        assert_eq!(tribal_weight(&magda, "tribal:Dwarf"), Some(2));
        assert_eq!(tribal_weight(&magda, "tribal:Berserker"), Some(1));
        assert_eq!(tribal_weight(&grunt, "tribal:Dwarf"), Some(1));
    }

    #[test]
    fn plural_and_irregular_subtype_forms_are_recognized() {
        let heritage = card(
            "Heritage Druid",
            "Tap three untapped Elves you control: Add {G}{G}{G}.",
            &["Creature"],
            &["Elf", "Druid"],
        );
        let goblins = card(
            "Goblin Rally Banner",
            "Goblins you control get +1/+1.",
            &["Artifact"],
            &[],
        );
        assert_eq!(tribal_weight(&heritage, "tribal:Elf"), Some(2));
        assert_eq!(tribal_weight(&goblins, "tribal:Goblin"), Some(2));
    }

    #[test]
    fn the_card_own_name_does_not_name_its_subtype() {
        let c = card(
            "Goblin Guide",
            "Haste\nWhenever Goblin Guide attacks, defending player reveals the top card of their library.",
            &["Creature"],
            &["Goblin", "Scout"],
        );
        assert_eq!(tribal_weight(&c, "tribal:Goblin"), Some(1));
    }

    #[test]
    fn reminder_text_and_negations_do_not_name_a_subtype() {
        let coat = card(
            "Coat of Arms",
            "Each creature gets +1/+1 for each other creature on the battlefield that shares at \
             least one creature type with it. (For example, if two Goblin Warriors and a Goblin \
             Shaman are on the battlefield, each gets +2/+2.)",
            &["Artifact"],
            &[],
        );
        let lord = card(
            "Mikaeus, the Unhallowed",
            "Other non-Human creatures you control get +1/+1 and have undying.",
            &["Creature"],
            &["Zombie", "Cleric"],
        );
        assert!(
            detect_themes(&coat).is_empty(),
            "{:?}",
            detect_themes(&coat)
        );
        assert_eq!(
            detect_themes(&lord),
            vec!["tribal:Zombie".to_string(), "tribal:Cleric".to_string()]
        );
    }

    #[test]
    fn a_theme_correction_has_the_plain_weight() {
        let mut c = card(
            "Magda, Brazen Outlaw",
            "Other Dwarves you control get +1/+0.",
            &["Creature"],
            &["Dwarf"],
        );
        c.corrections.themes = Some(vec!["tribal:Dwarf".to_string()]);
        assert_eq!(
            detect_weighted_themes(&c),
            vec![("tribal:Dwarf".to_string(), 1)]
        );
    }

    #[test]
    fn a_theme_correction_replaces_the_detected_themes() {
        let mut c = card("Goblin Chieftain", "", &["Creature"], &["Goblin"]);
        c.corrections.themes = Some(vec!["tokens".to_string()]);
        assert_eq!(detect_themes(&c), vec!["tokens".to_string()]);
    }
}
