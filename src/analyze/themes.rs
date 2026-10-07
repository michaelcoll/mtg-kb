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

/// Thème et motif sur le texte oracle d'une Face. Les motifs visent les
/// payoffs et enablers d'une stratégie, pas la simple mention d'un mot (une
/// Carte qui détruit un artefact n'est pas une Carte artefacts).
static THEME_PATTERNS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    [
        // Tous les jetons, Treasure, Clue et Food compris.
        ("tokens", r"(?i)creates? .*tokens?"),
        ("+1/+1", r"\+1/\+1 counters?"),
        (
            "aristocrats",
            r"(?i)(whenever .* dies|sacrifice (a|an|another) creature)",
        ),
        (
            "artefacts",
            r"(?i)(artifacts? you control|artifact spells?|whenever (you cast )?an(other)? artifact|for each artifact|number of artifacts|affinity for artifacts|metalcraft|improvise)",
        ),
        (
            "enchantements",
            r"(?i)(enchantments? you control|enchantment spells?|whenever (you cast )?an(other)? enchantment|for each enchantment|number of enchantments|constellation)",
        ),
        (
            "spellslinger",
            r"(?i)(instant (and|or) sorcery spells?|instants? and sorcer(y|ies) you cast|copy target (instant |sorcery )?spell|magecraft)",
        ),
        // Réanimation, jeu depuis le cimetière, mill de soi (« mill three
        // cards » à l'impératif ; « that player mills » vise un adversaire).
        (
            "cimetiere",
            r"(?i)(from (a|your) graveyard (onto|to) the battlefield|(cast|play) [^.]* from your graveyard|(^|[.,:—]\s*)mill (a|one|two|three|four|five|six|seven|eight|nine|ten|x|\d+) cards?|\byou mill\b)",
        ),
        (
            "landfall",
            r"(?i)(landfall|whenever (a|one or more) lands? (you control )?enters?|play (an )?additional lands?)",
        ),
        // Payoffs du gain de vie, pas le simple « you gain 1 life ».
        (
            "lifegain",
            r"(?i)(whenever you gain life|if you would gain life|you('ve)? gained life|life you gained|have lifelink)",
        ),
        // Flicker (immédiat ou en fin de tour) et doublement des effets
        // d'arrivée ; pas l'exil « tant que » d'un Oblivion Ring.
        (
            "blink",
            r"(?i)(exile [^.]*, then return (it|that card|them|those cards)[^.]* to the battlefield|exile (another )?target [^.]*\.\s*return (it|that card|them|those cards) to the battlefield|entering the battlefield causes a triggered ability)",
        ),
        // Équipements et auras qui renforcent une créature, pas une aura de
        // contrôle comme Pacifism.
        (
            "voltron",
            r"(?i)(equipped creature|equipment you control|auras? you control|aura spells?|for each (aura|equipment)|enchanted creature gets \+)",
        ),
    ]
    .into_iter()
    .map(|(theme, pattern)| (theme, Regex::new(pattern).unwrap()))
    .collect()
});

fn detect_face_themes(face: &Face) -> Vec<String> {
    let text = face.oracle_text.as_deref().unwrap_or("");
    let mut themes: Vec<String> = THEME_PATTERNS
        .iter()
        .filter(|(_, pattern)| pattern.is_match(text))
        .map(|(theme, _)| theme.to_string())
        .collect();

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
    fn noncreature_tokens_keep_the_tokens_theme() {
        let c = card(
            "Dockside Extortionist",
            "When Dockside Extortionist enters, create X Treasure tokens, where X is the number of artifacts and enchantments your opponents control.",
            &["Creature"],
            &["Goblin", "Pirate"],
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

    fn has_theme(card: &Card, theme: &str) -> bool {
        detect_themes(card).iter().any(|t| t == theme)
    }

    #[test]
    fn detects_artefacts_theme() {
        let c = card(
            "Etherium Sculptor",
            "Artifact spells you cast cost {1} less to cast.",
            &["Artifact", "Creature"],
            &["Vedalken", "Artificer"],
        );
        assert!(has_theme(&c, "artefacts"));
    }

    #[test]
    fn destroying_an_artifact_is_not_the_artefacts_theme() {
        let c = card(
            "Naturalize",
            "Destroy target artifact or enchantment.",
            &["Instant"],
            &[],
        );
        assert!(!has_theme(&c, "artefacts"));
    }

    #[test]
    fn detects_enchantements_theme() {
        let c = card(
            "Enchantress's Presence",
            "Whenever you cast an enchantment spell, draw a card.",
            &["Enchantment"],
            &[],
        );
        assert!(has_theme(&c, "enchantements"));
    }

    #[test]
    fn destroying_an_enchantment_is_not_the_enchantements_theme() {
        let c = card(
            "Naturalize",
            "Destroy target artifact or enchantment.",
            &["Instant"],
            &[],
        );
        assert!(!has_theme(&c, "enchantements"));
    }

    #[test]
    fn detects_spellslinger_theme_on_instant_and_sorcery_triggers() {
        let c = card(
            "Young Pyromancer",
            "Whenever you cast an instant or sorcery spell, create a 1/1 red Elemental creature token.",
            &["Creature"],
            &["Human", "Shaman"],
        );
        assert!(has_theme(&c, "spellslinger"));
    }

    #[test]
    fn detects_spellslinger_theme_on_spell_copy() {
        let c = card(
            "Reverberate",
            "Copy target instant or sorcery spell. You may choose new targets for the copy.",
            &["Instant"],
            &[],
        );
        assert!(has_theme(&c, "spellslinger"));
    }

    #[test]
    fn a_mere_instant_is_not_the_spellslinger_theme() {
        let c = card(
            "Lightning Bolt",
            "Lightning Bolt deals 3 damage to any target.",
            &["Instant"],
            &[],
        );
        assert!(!has_theme(&c, "spellslinger"));
    }

    #[test]
    fn detects_cimetiere_theme_on_reanimation() {
        let c = card(
            "Reanimate",
            "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to its mana value.",
            &["Sorcery"],
            &[],
        );
        assert!(has_theme(&c, "cimetiere"));
    }

    #[test]
    fn detects_cimetiere_theme_on_casting_from_the_graveyard() {
        let c = card(
            "Lurrus of the Dream-Den",
            "During each of your turns, you may cast one permanent spell with mana value 2 or less from your graveyard.",
            &["Creature"],
            &["Cat", "Nightmare"],
        );
        assert!(has_theme(&c, "cimetiere"));
    }

    #[test]
    fn detects_cimetiere_theme_on_self_mill() {
        let c = card(
            "Stitcher's Supplier",
            "When Stitcher's Supplier enters or dies, mill three cards.",
            &["Creature"],
            &["Zombie"],
        );
        assert!(has_theme(&c, "cimetiere"));
    }

    #[test]
    fn graveyard_hate_is_not_the_cimetiere_theme() {
        let c = card(
            "Bojuka Bog",
            "Bojuka Bog enters tapped.\nWhen Bojuka Bog enters, exile target player's graveyard.\n{T}: Add {B}.",
            &["Land"],
            &[],
        );
        assert!(!has_theme(&c, "cimetiere"));
    }

    #[test]
    fn milling_an_opponent_is_not_the_cimetiere_theme() {
        let c = card(
            "Mindcrank",
            "Whenever an opponent loses life, that player mills that many cards.",
            &["Artifact"],
            &[],
        );
        assert!(!has_theme(&c, "cimetiere"));
    }

    #[test]
    fn detects_landfall_theme() {
        let c = card(
            "Lotus Cobra",
            "Landfall — Whenever a land you control enters, add one mana of any color.",
            &["Creature"],
            &["Snake"],
        );
        assert!(has_theme(&c, "landfall"));
    }

    #[test]
    fn detects_landfall_theme_on_extra_land_drops() {
        let c = card(
            "Exploration",
            "You may play an additional land on each of your turns.",
            &["Enchantment"],
            &[],
        );
        assert!(has_theme(&c, "landfall"));
    }

    #[test]
    fn an_opponents_land_entering_is_not_the_landfall_theme() {
        let c = card(
            "Tunnel Ignus",
            "Whenever a land an opponent controls enters, if that player had another land enter the battlefield under their control this turn, Tunnel Ignus deals 3 damage to that player.",
            &["Creature"],
            &["Elemental"],
        );
        assert!(!has_theme(&c, "landfall"));
    }

    #[test]
    fn detects_lifegain_theme() {
        let c = card(
            "Ajani's Pridemate",
            "Whenever you gain life, put a +1/+1 counter on Ajani's Pridemate.",
            &["Creature"],
            &["Cat", "Soldier"],
        );
        assert!(has_theme(&c, "lifegain"));
    }

    #[test]
    fn incidental_life_gain_is_not_the_lifegain_theme() {
        let c = card(
            "Blood Artist",
            "Whenever Blood Artist or another creature dies, target player loses 1 life and you gain 1 life.",
            &["Creature"],
            &["Vampire"],
        );
        assert!(!has_theme(&c, "lifegain"));
    }

    #[test]
    fn detects_blink_theme_on_immediate_flicker() {
        let c = card(
            "Ephemerate",
            "Exile target creature you control, then return it to the battlefield under its owner's control.\nRebound",
            &["Instant"],
            &[],
        );
        assert!(has_theme(&c, "blink"));
    }

    #[test]
    fn detects_blink_theme_on_delayed_flicker() {
        let c = card(
            "Flickerwisp",
            "Flying\nWhen Flickerwisp enters, exile another target permanent. Return that card to the battlefield under its owner's control at the beginning of the next end step.",
            &["Creature"],
            &["Elemental"],
        );
        assert!(has_theme(&c, "blink"));
    }

    #[test]
    fn detects_blink_theme_on_enter_trigger_doubling() {
        let c = card(
            "Panharmonicon",
            "If an artifact or creature entering the battlefield causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.",
            &["Artifact"],
            &[],
        );
        assert!(has_theme(&c, "blink"));
    }

    #[test]
    fn exile_until_leaves_is_not_the_blink_theme() {
        let c = card(
            "Oblivion Ring",
            "When Oblivion Ring enters, exile another target nonland permanent.\nWhen Oblivion Ring leaves the battlefield, return the exiled card to the battlefield under its owner's control.",
            &["Enchantment"],
            &[],
        );
        assert!(!has_theme(&c, "blink"));
    }

    #[test]
    fn detects_voltron_theme_on_equipment() {
        let c = card(
            "Sword of Fire and Ice",
            "Equipped creature gets +2/+2 and has protection from red and from blue.\nEquip {2}",
            &["Artifact"],
            &["Equipment"],
        );
        assert!(has_theme(&c, "voltron"));
    }

    #[test]
    fn detects_voltron_theme_on_auras() {
        let c = card(
            "Ethereal Armor",
            "Enchant creature\nEnchanted creature gets +1/+1 for each enchantment you control and has first strike.",
            &["Enchantment"],
            &["Aura"],
        );
        assert!(has_theme(&c, "voltron"));
    }

    #[test]
    fn a_removal_aura_is_not_the_voltron_theme() {
        let c = card(
            "Pacifism",
            "Enchant creature\nEnchanted creature can't attack or block.",
            &["Enchantment"],
            &["Aura"],
        );
        assert!(!has_theme(&c, "voltron"));
    }

    #[test]
    fn a_multi_face_card_has_the_union_of_its_new_themes() {
        let c = Card {
            back: Some(face(
                "Ormos, Archive Keeper",
                "Whenever you cast an instant or sorcery spell, draw a card.",
                &["Creature"],
                &[],
            )),
            ..card(
                "Front Face",
                "Landfall — Whenever a land you control enters, you gain 1 life.",
                &["Sorcery"],
                &[],
            )
        };
        let themes = detect_themes(&c);
        assert!(themes.contains(&"landfall".to_string()), "{themes:?}");
        assert!(themes.contains(&"spellslinger".to_string()), "{themes:?}");
    }

    #[test]
    fn a_theme_correction_replaces_new_detected_themes_too() {
        let mut c = card(
            "Etherium Sculptor",
            "Artifact spells you cast cost {1} less to cast.",
            &["Artifact", "Creature"],
            &[],
        );
        c.corrections.themes = Some(vec![]);
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
