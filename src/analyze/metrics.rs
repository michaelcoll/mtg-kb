use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Card, Face, ManaBase, ManaCurve, ManaCurveBucket, ResolvedCard};

const COLORS: [&str; 5] = ["W", "U", "B", "R", "G"];

/// Une Correction de Rôles remplace la détection (ADR 0005).
pub fn detect_roles(card: &Card) -> Vec<String> {
    match &card.corrections.roles {
        Some(roles) => roles.clone(),
        None => card.union_over_faces(detect_face_roles),
    }
}

fn detect_face_roles(face: &Face) -> Vec<String> {
    static RAMP_LAND_SEARCH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)search your library for (a|up to \w+|\w+) .*land").unwrap()
    });
    static MANA_ABILITY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)add\b[^.]*(mana|\{)").unwrap());
    static DRAW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)draws? (cards? equal to|that many cards?|x cards?|(a|an|one|two|three|four|five|\d+)( additional)? cards?)",
        )
        .unwrap()
    });
    static TARGETED_REMOVAL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)((destroy|exile)( up to)?( (one|two|three|four|five|x|\d+))? target|target creature gets -\d|shuffles? it into (its|their)( owner'?s?)? library|on the (top|bottom) of its owner'?s library|deals? (\d+|x) damage to (any target|target creature|target planeswalker))",
        )
        .unwrap()
    });
    static WIPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)(destroy all|exile all|each creature (gets|deals)|all creatures get|deals? (\d+|x) damage to each creature)",
        )
        .unwrap()
    });
    static PROTECTION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(hexproof|indestructible|protection from)").unwrap());
    // Ce qui est cherché, jusqu'au mot "card" ; une recherche de terrain est du ramp.
    static LIBRARY_SEARCH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)search your library (?:and/or graveyard )?for ([^.]*?)\bcards?\b").unwrap()
    });
    static LAND_WORD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(lands?|plains|islands?|swamps?|mountains?|forests?)\b").unwrap()
    });
    static RECURSION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(return|put) [^.]*?\bfrom (your|a) graveyard (to|onto)\b").unwrap()
    });
    // Exil depuis le cimetière d'un autre ou de tous ; pas l'exil de son propre cimetière.
    static GRAVE_HATE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)exile [^.]*?\b(a|all|target player's|target opponent's|each opponent's|each player's|that player's) graveyards?\b",
        )
        .unwrap()
    });
    static COUNTERSPELL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)counter target [^.]*?\b(spell|ability)").unwrap());
    // Haine de cimetière : cible une carte/le cimetière, pas un removal de permanent.
    static GRAVEYARD_TARGET: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)(target[\w ']*graveyard|all graveyards)").unwrap());
    // Destruction de terrain seul : pas du removal de permanent au sens "menace".
    static LAND_ONLY_TARGET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(destroy|exile)( up to (one|two|three|four|five|x|\d+))? target lands?\b")
            .unwrap()
    });

    let mut roles = Vec::new();
    let text = face.oracle_text.as_deref().unwrap_or("");

    if face.is_land() {
        roles.push("terrain".to_string());
    }
    if !face.is_land() && (RAMP_LAND_SEARCH.is_match(text) || MANA_ABILITY.is_match(text)) {
        roles.push("ramp".to_string());
    }
    if DRAW.is_match(text) {
        roles.push("pioche".to_string());
    }
    let is_graveyard_hate = GRAVEYARD_TARGET.is_match(text);
    let is_land_only_destruction = LAND_ONLY_TARGET.is_match(text);
    if !is_graveyard_hate && !is_land_only_destruction && TARGETED_REMOVAL.is_match(text) {
        roles.push("removal_cible".to_string());
    }
    if !is_graveyard_hate && !is_land_only_destruction && WIPE.is_match(text) {
        roles.push("wipe".to_string());
    }
    if PROTECTION.is_match(text)
        || face
            .keywords
            .iter()
            .any(|k| k == "Hexproof" || k == "Indestructible")
    {
        roles.push("protection".to_string());
    }
    if LIBRARY_SEARCH
        .captures_iter(text)
        .any(|caps| !LAND_WORD.is_match(&caps[1]))
    {
        roles.push("tutor".to_string());
    }
    if RECURSION.is_match(text) {
        roles.push("recursion".to_string());
    }
    if GRAVE_HATE.is_match(text) {
        roles.push("grave_hate".to_string());
    }
    if COUNTERSPELL.is_match(text) {
        roles.push("contresort".to_string());
    }
    roles
}

/// Hors terrains ; le bucket 7 regroupe 7 et plus.
pub fn mana_curve(cards: &[ResolvedCard]) -> ManaCurve {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    let mut weighted_total = 0.0;
    let mut nonland_count = 0u32;

    for resolved in cards {
        if resolved.card.is_land() {
            continue;
        }
        let mv = resolved.card.mana_value.unwrap_or(0.0);
        let bucket = (mv.floor() as i64).clamp(0, 7) as u32;
        *counts.entry(bucket).or_insert(0) += resolved.quantity;
        weighted_total += mv * resolved.quantity as f64;
        nonland_count += resolved.quantity;
    }

    let buckets = counts
        .into_iter()
        .map(|(mana_value, count)| ManaCurveBucket { mana_value, count })
        .collect();
    let average_mana_value = if nonland_count > 0 {
        (weighted_total / nonland_count as f64 * 100.0).round() / 100.0
    } else {
        0.0
    };

    ManaCurve {
        buckets,
        average_mana_value,
    }
}

/// Un symbole hybride "{W/U}" compte pour W et pour U.
pub fn count_color_symbols(mana_cost: &str, counts: &mut BTreeMap<String, u32>) {
    static SYMBOL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]+)\}").unwrap());
    for caps in SYMBOL.captures_iter(mana_cost) {
        let symbol = caps.get(1).unwrap().as_str();
        for color in COLORS {
            if symbol.contains(color) {
                *counts.entry(color.to_string()).or_insert(0) += 1;
            }
        }
    }
}

fn land_color_sources(
    card: &Card,
    quantity: u32,
    commander_color_identity: &[String],
    counts: &mut BTreeMap<String, u32>,
) {
    for color in land_colors(card, commander_color_identity) {
        *counts.entry(color.to_string()).or_insert(0) += quantity;
    }
}

/// Couleurs que produisent les Faces terrain de `card`.
pub fn land_colors(card: &Card, commander_color_identity: &[String]) -> HashSet<&'static str> {
    static ADD_CLAUSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)add ([^.;]*)").unwrap());
    static SYMBOL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]+)\}").unwrap());
    static ADD_ANY_COLOR_COLOR_IDENTITY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)add (one|a) mana of any color in your commander'?s? color identity")
            .unwrap()
    });
    // Sans référence à l'Identité de couleur (Exotic Orchard…) : les 5 couleurs.
    static ADD_ANY_COLOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)add (one|a) mana of any color").unwrap());
    let mut colors_seen = HashSet::new();
    for face in card.faces().filter(|f| f.is_land()) {
        let text = face.oracle_text.as_deref().unwrap_or("");
        for add_caps in ADD_CLAUSE.captures_iter(text) {
            let clause = add_caps.get(1).unwrap().as_str();
            for symbol_caps in SYMBOL.captures_iter(clause) {
                let symbol = symbol_caps.get(1).unwrap().as_str();
                for color in COLORS {
                    if symbol.contains(color) {
                        colors_seen.insert(color);
                    }
                }
            }
        }
        if ADD_ANY_COLOR_COLOR_IDENTITY.is_match(text) {
            for color in COLORS {
                if commander_color_identity.iter().any(|c| c == color) {
                    colors_seen.insert(color);
                }
            }
        } else if ADD_ANY_COLOR.is_match(text) {
            colors_seen.extend(COLORS);
        }
    }
    colors_seen
}

pub fn mana_base(cards: &[ResolvedCard], commander: &Card) -> ManaBase {
    let mut land_count = 0u32;
    let mut sources_by_color: BTreeMap<String, u32> = BTreeMap::new();
    let mut symbols_by_color: BTreeMap<String, u32> = BTreeMap::new();

    for resolved in cards {
        if resolved.card.is_land() {
            land_count += resolved.quantity;
            land_color_sources(
                &resolved.card,
                resolved.quantity,
                &commander.color_identity,
                &mut sources_by_color,
            );
        }
        for face in resolved.card.faces().filter(|f| !f.is_land()) {
            if let Some(cost) = &face.mana_cost {
                count_color_symbols(cost, &mut symbols_by_color);
            }
        }
    }
    for face in commander.faces() {
        if let Some(cost) = &face.mana_cost {
            count_color_symbols(cost, &mut symbols_by_color);
        }
    }

    ManaBase {
        land_count,
        sources_by_color,
        symbols_by_color,
    }
}

/// Seuils de `kb analyze`, exposés tels quels en options de la CLI : les
/// défauts ne sont écrits qu'ici, dans `Thresholds::DEFAULT`. Inscrits dans
/// le JSON pour que `kb report` juge les échanges sur les mêmes seuils ; un
/// seuil absent (JSON ancien) reprend son défaut.
#[derive(Debug, Clone, PartialEq, clap::Args, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// Nombre minimal de terrains attendu
    #[arg(long, default_value_t = Thresholds::DEFAULT.min_lands)]
    pub min_lands: u32,
    /// Nombre minimal de Cartes de Rôle "ramp" attendu
    #[arg(long, default_value_t = Thresholds::DEFAULT.min_ramp)]
    pub min_ramp: u32,
    /// Nombre minimal de Cartes de Rôle "pioche" attendu
    #[arg(long, default_value_t = Thresholds::DEFAULT.min_draw)]
    pub min_draw: u32,
    /// Nombre minimal de Cartes de Rôle "removal_cible" attendu
    #[arg(long, default_value_t = Thresholds::DEFAULT.min_removal)]
    pub min_removal: u32,
    /// Nombre minimal de Cartes de Rôle "wipe" attendu
    #[arg(long, default_value_t = Thresholds::DEFAULT.min_wipe)]
    pub min_wipe: u32,
    /// Mana value moyenne (hors terrains) au-delà de laquelle la courbe
    /// est jugée trop chère
    #[arg(long, default_value_t = Thresholds::DEFAULT.max_average_mana_value)]
    pub max_average_mana_value: f64,
    /// Nombre de Cartes à mana value ≥ 6 au-delà duquel la courbe est
    /// jugée déséquilibrée vers le haut
    #[arg(long, default_value_t = Thresholds::DEFAULT.max_high_cost_cards)]
    pub max_high_cost_cards: u32,
    /// Écart, en points de pourcentage, entre la part de symboles de mana et
    /// la part de sources (terrains) d'une couleur de l'Identité au-delà
    /// duquel cette couleur est jugée sous-alimentée
    #[arg(long, default_value_t = Thresholds::DEFAULT.max_color_source_gap)]
    pub max_color_source_gap: f64,
}

impl Thresholds {
    pub const DEFAULT: Self = Self {
        min_lands: 35,
        min_ramp: 10,
        min_draw: 8,
        min_removal: 8,
        min_wipe: 2,
        max_average_mana_value: 3.5,
        max_high_cost_cards: 8,
        max_color_source_gap: 10.0,
    };
}

impl Default for Thresholds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Thresholds {
    /// Minimum de chaque Rôle à seuil, `terrain` compris.
    pub fn role_minimums(&self) -> [(&'static str, u32); 5] {
        [
            (TERRAIN, self.min_lands),
            (RAMP, self.min_ramp),
            (PIOCHE, self.min_draw),
            (REMOVAL_CIBLE, self.min_removal),
            (WIPE, self.min_wipe),
        ]
    }
}

const HIGH_COST_MANA_VALUE: u32 = 6;

pub fn curve_weaknesses(curve: &ManaCurve, thresholds: &Thresholds) -> Vec<String> {
    let mut weaknesses = Vec::new();

    if curve.average_mana_value > thresholds.max_average_mana_value {
        weaknesses.push(format!(
            "courbe de mana déséquilibrée : mana value moyenne de {} (> {})",
            curve.average_mana_value, thresholds.max_average_mana_value
        ));
    }
    let high_cost_count: u32 = curve
        .buckets
        .iter()
        .filter(|b| b.mana_value >= HIGH_COST_MANA_VALUE)
        .map(|b| b.count)
        .sum();
    if high_cost_count > thresholds.max_high_cost_cards {
        weaknesses.push(format!(
            "courbe de mana déséquilibrée : {high_cost_count} cartes à {HIGH_COST_MANA_VALUE}+ de mana value (> {})",
            thresholds.max_high_cost_cards
        ));
    }
    weaknesses
}

/// Couleur de l'Identité dont la part de sources est inférieure de plus de
/// `max_color_source_gap` points à sa part de symboles de mana.
struct UndersuppliedColor {
    color: &'static str,
    symbol_percent: f64,
    source_percent: f64,
}

/// Part de symboles : parmi les symboles des couleurs de l'Identité. Part de
/// sources : parmi les terrains (un dual compte pour ses deux couleurs). Un
/// Deck monocolore n'a jamais de couleur sous-alimentée.
fn find_undersupplied_colors(
    mana_base: &ManaBase,
    commander_color_identity: &[String],
    thresholds: &Thresholds,
) -> Vec<UndersuppliedColor> {
    let identity: Vec<&'static str> = COLORS
        .into_iter()
        .filter(|color| commander_color_identity.iter().any(|c| c == color))
        .collect();
    let count = |counts: &BTreeMap<String, u32>, color: &str| *counts.get(color).unwrap_or(&0);
    let total_symbols: u32 = identity
        .iter()
        .map(|color| count(&mana_base.symbols_by_color, color))
        .sum();
    if identity.len() < 2 || total_symbols == 0 || mana_base.land_count == 0 {
        return Vec::new();
    }
    let percent = |part: u32, total: u32| f64::from(part * 100) / f64::from(total);

    identity
        .into_iter()
        .map(|color| UndersuppliedColor {
            color,
            symbol_percent: percent(count(&mana_base.symbols_by_color, color), total_symbols),
            source_percent: percent(
                count(&mana_base.sources_by_color, color),
                mana_base.land_count,
            ),
        })
        .filter(|c| c.symbol_percent - c.source_percent > thresholds.max_color_source_gap)
        .collect()
}

/// Couleurs sous-alimentées, dans l'ordre WUBRG.
pub fn undersupplied_colors(
    mana_base: &ManaBase,
    commander_color_identity: &[String],
    thresholds: &Thresholds,
) -> Vec<String> {
    find_undersupplied_colors(mana_base, commander_color_identity, thresholds)
        .into_iter()
        .map(|c| c.color.to_string())
        .collect()
}

pub fn color_weaknesses(
    mana_base: &ManaBase,
    commander_color_identity: &[String],
    thresholds: &Thresholds,
) -> Vec<String> {
    find_undersupplied_colors(mana_base, commander_color_identity, thresholds)
        .into_iter()
        .map(|c| {
            format!(
                "couleur sous-alimentée : {} porte {:.0} % des symboles de mana mais {:.0} % des \
                 sources (écart > {} points)",
                c.color, c.symbol_percent, c.source_percent, thresholds.max_color_source_gap
            )
        })
        .collect()
}

const TERRAIN: &str = "terrain";
const RAMP: &str = "ramp";
const PIOCHE: &str = "pioche";
const REMOVAL_CIBLE: &str = "removal_cible";
const WIPE: &str = "wipe";

fn role_thresholds(thresholds: &Thresholds) -> [(&'static str, &'static str, u32); 4] {
    [
        (RAMP, "Ramp", thresholds.min_ramp),
        (PIOCHE, "Pioche", thresholds.min_draw),
        (REMOVAL_CIBLE, "Removal ciblé", thresholds.min_removal),
        (WIPE, "Wipe", thresholds.min_wipe),
    ]
}

/// Rôles sous leur seuil, qui ouvrent chacun un panier de Candidats :
/// `terrain` sous `min_lands` (Point faible « base de mana insuffisante »),
/// puis les Rôles de `role_thresholds`.
pub fn weak_role_names(
    role_counts: &BTreeMap<String, u32>,
    thresholds: &Thresholds,
) -> Vec<String> {
    let lands = (TERRAIN, thresholds.min_lands);
    let roles = role_thresholds(thresholds).map(|(role, _, min)| (role, min));
    std::iter::once(lands)
        .chain(roles)
        .filter(|(role, min)| *role_counts.get(*role).unwrap_or(&0) < *min)
        .map(|(role, _)| role.to_string())
        .collect()
}

pub fn role_weaknesses(
    role_counts: &BTreeMap<String, u32>,
    land_count: u32,
    thresholds: &Thresholds,
) -> Vec<String> {
    let mut weaknesses = Vec::new();

    if land_count < thresholds.min_lands {
        weaknesses.push(format!(
            "base de mana insuffisante : {land_count} terrains (< {})",
            thresholds.min_lands
        ));
    }
    for (role, label, min) in role_thresholds(thresholds) {
        let count = *role_counts.get(role).unwrap_or(&0);
        if count < min {
            weaknesses.push(format!(
                "{label} sous-représenté : {count} cartes (< {min})"
            ));
        }
    }
    weaknesses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Card;

    fn face(name: &str, oracle_text: &str, types: &[&str], mana_cost: Option<&str>) -> Face {
        Face {
            name: name.to_string(),
            mana_cost: mana_cost.map(str::to_string),
            types: types.iter().map(|t| t.to_string()).collect(),
            oracle_text: Some(oracle_text.to_string()),
            ..Face::default()
        }
    }

    fn card(name: &str, oracle_text: &str, types: &[&str], mana_cost: Option<&str>) -> Card {
        Card::from_face(face(name, oracle_text, types, mana_cost))
    }

    fn bala_ged_recovery() -> Card {
        Card {
            back: Some(face(
                "Bala Ged Sanctuary",
                "As Bala Ged Sanctuary enters, you may pay 3 life.\n{T}: Add {G}.",
                &["Land"],
                None,
            )),
            ..card(
                "Bala Ged Recovery",
                "Return target card from your graveyard to your hand.",
                &["Sorcery"],
                Some("{2}{G}"),
            )
        }
    }

    #[test]
    fn a_multi_face_card_has_the_union_of_its_faces_roles() {
        let pathway = Card {
            back: Some(face("Back", "Draw two cards.", &["Sorcery"], None)),
            ..card("Front", "Destroy target creature.", &["Instant"], None)
        };
        assert_eq!(
            detect_roles(&pathway),
            vec!["removal_cible".to_string(), "pioche".to_string()]
        );
    }

    fn bala_ged_recovery_mana_base() -> ManaBase {
        let resolved = ResolvedCard {
            quantity: 1,
            roles: vec![],
            themes: vec![],
            card: bala_ged_recovery(),
        };
        mana_base(&[resolved], &card("Commander", "", &["Creature"], None))
    }

    #[test]
    fn a_spell_land_modal_card_counts_as_a_land_and_a_green_source() {
        let base = bala_ged_recovery_mana_base();
        assert_eq!(base.land_count, 1);
        assert_eq!(base.sources_by_color.get("G"), Some(&1));
    }

    #[test]
    fn a_spell_land_modal_card_counts_the_spell_face_symbols() {
        let base = bala_ged_recovery_mana_base();
        assert_eq!(base.symbols_by_color.get("G"), Some(&1));
    }

    #[test]
    fn detects_terrain_role_from_land_type() {
        let land = card("Forest", "", &["Land"], None);
        assert_eq!(detect_roles(&land), vec!["terrain".to_string()]);
    }

    #[test]
    fn detects_ramp_from_mana_ability() {
        let sol_ring = card("Sol Ring", "{T}: Add {C}{C}.", &["Artifact"], Some("{1}"));
        assert!(detect_roles(&sol_ring).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_ramp_from_land_search() {
        let rampant = card(
            "Rampant Growth",
            "Search your library for a basic land card and put it onto the battlefield tapped.",
            &["Sorcery"],
            Some("{1}{G}"),
        );
        assert!(detect_roles(&rampant).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_draw_removal_wipe_protection() {
        let draw = card("Divination", "Draw two cards.", &["Sorcery"], None);
        assert!(detect_roles(&draw).contains(&"pioche".to_string()));

        let removal = card("Doom Blade", "Destroy target creature.", &["Instant"], None);
        assert!(detect_roles(&removal).contains(&"removal_cible".to_string()));

        let wipe = card("Wrath of God", "Destroy all creatures.", &["Sorcery"], None);
        assert!(detect_roles(&wipe).contains(&"wipe".to_string()));

        let protection = card(
            "Swiftfoot Boots",
            "Equipped creature has hexproof and haste.",
            &["Artifact"],
            None,
        );
        assert!(detect_roles(&protection).contains(&"protection".to_string()));
    }

    #[test]
    fn a_counterspell_is_contresort_and_not_protection() {
        let counterspell = card(
            "Counterspell",
            "Counter target spell.",
            &["Instant"],
            Some("{U}{U}"),
        );
        assert_eq!(detect_roles(&counterspell), vec!["contresort".to_string()]);

        let swan_song = card(
            "Swan Song",
            "Counter target enchantment, instant, or sorcery spell. Its controller creates a 2/2 blue Bird creature token with flying.",
            &["Instant"],
            Some("{U}"),
        );
        assert_eq!(detect_roles(&swan_song), vec!["contresort".to_string()]);

        let stifle = card(
            "Stifle",
            "Counter target activated or triggered ability. (Mana abilities can't be targeted.)",
            &["Instant"],
            Some("{U}"),
        );
        assert_eq!(detect_roles(&stifle), vec!["contresort".to_string()]);
    }

    #[test]
    fn a_nonland_library_search_is_a_tutor() {
        let demonic_tutor = card(
            "Demonic Tutor",
            "Search your library for a card, put that card into your hand, then shuffle.",
            &["Sorcery"],
            Some("{1}{B}"),
        );
        assert_eq!(detect_roles(&demonic_tutor), vec!["tutor".to_string()]);

        let worldly_tutor = card(
            "Worldly Tutor",
            "Search your library for a creature card, reveal it, then shuffle and put the card on top.",
            &["Instant"],
            Some("{G}"),
        );
        assert_eq!(detect_roles(&worldly_tutor), vec!["tutor".to_string()]);

        let enlightened_tutor = card(
            "Enlightened Tutor",
            "Search your library for an artifact or enchantment card, reveal it, then shuffle and put that card on top.",
            &["Instant"],
            Some("{W}"),
        );
        assert_eq!(detect_roles(&enlightened_tutor), vec!["tutor".to_string()]);
    }

    #[test]
    fn a_land_search_is_not_a_tutor() {
        let cultivate = card(
            "Cultivate",
            "Search your library for up to two basic land cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.",
            &["Sorcery"],
            Some("{2}{G}"),
        );
        assert!(!detect_roles(&cultivate).contains(&"tutor".to_string()));

        let farseek = card(
            "Farseek",
            "Search your library for an Island, Swamp, Mountain, or Plains card and put it onto the battlefield tapped. Then shuffle.",
            &["Sorcery"],
            Some("{1}{G}"),
        );
        assert!(!detect_roles(&farseek).contains(&"tutor".to_string()));

        let evolving_wilds = card(
            "Evolving Wilds",
            "{T}, Sacrifice Evolving Wilds: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            &["Land"],
            None,
        );
        assert!(!detect_roles(&evolving_wilds).contains(&"tutor".to_string()));
    }

    #[test]
    fn returning_a_card_from_the_graveyard_is_recursion() {
        let regrowth = card(
            "Regrowth",
            "Return target card from your graveyard to your hand.",
            &["Sorcery"],
            Some("{1}{G}"),
        );
        assert_eq!(detect_roles(&regrowth), vec!["recursion".to_string()]);

        let reanimate = card(
            "Reanimate",
            "Put target creature card from a graveyard onto the battlefield under your control. You lose life equal to its mana value.",
            &["Sorcery"],
            Some("{B}"),
        );
        assert_eq!(detect_roles(&reanimate), vec!["recursion".to_string()]);

        let eternal_witness = card(
            "Eternal Witness",
            "When this creature enters, you may return target card from your graveyard to your hand.",
            &["Creature"],
            Some("{1}{G}{G}"),
        );
        assert_eq!(
            detect_roles(&eternal_witness),
            vec!["recursion".to_string()]
        );
    }

    #[test]
    fn exiling_from_a_graveyard_is_not_recursion() {
        let bojuka_bog = card(
            "Bojuka Bog",
            "Bojuka Bog enters tapped.\nWhen Bojuka Bog enters, exile target player's graveyard.\n{T}: Add {B}.",
            &["Land"],
            None,
        );
        assert!(!detect_roles(&bojuka_bog).contains(&"recursion".to_string()));

        let crook_of_condemnation = card(
            "Crook of Condemnation",
            "{T}: Exile target card from a graveyard. If it was a creature card, you gain 1 life. {2}, {T}, Sacrifice Crook of Condemnation: Exile all graveyards.",
            &["Artifact"],
            Some("{2}"),
        );
        assert!(!detect_roles(&crook_of_condemnation).contains(&"recursion".to_string()));
    }

    #[test]
    fn exiling_cards_from_graveyards_is_grave_hate() {
        let rest_in_peace = card(
            "Rest in Peace",
            "When Rest in Peace enters, exile all graveyards.\nIf a card or token would be put into a graveyard from anywhere, exile it instead.",
            &["Enchantment"],
            Some("{1}{W}"),
        );
        assert_eq!(detect_roles(&rest_in_peace), vec!["grave_hate".to_string()]);

        let crook_of_condemnation = card(
            "Crook of Condemnation",
            "{T}: Exile target card from a graveyard. If it was a creature card, you gain 1 life. {2}, {T}, Sacrifice Crook of Condemnation: Exile all graveyards.",
            &["Artifact"],
            Some("{2}"),
        );
        assert_eq!(
            detect_roles(&crook_of_condemnation),
            vec!["grave_hate".to_string()]
        );

        let bojuka_bog = card(
            "Bojuka Bog",
            "Bojuka Bog enters tapped.\nWhen Bojuka Bog enters, exile target player's graveyard.\n{T}: Add {B}.",
            &["Land"],
            None,
        );
        assert!(detect_roles(&bojuka_bog).contains(&"grave_hate".to_string()));
    }

    #[test]
    fn exiling_from_your_own_graveyard_as_a_cost_is_not_grave_hate() {
        let drudge_spell = card(
            "Drudge Spell",
            "Exile two creature cards from your graveyard: Create a 1/1 black Skeleton creature token.",
            &["Enchantment"],
            Some("{B}{B}"),
        );
        assert!(!detect_roles(&drudge_spell).contains(&"grave_hate".to_string()));
    }

    #[test]
    fn protection_still_covers_hexproof_and_indestructible() {
        let heroic_intervention = card(
            "Heroic Intervention",
            "Permanents you control gain hexproof and indestructible until end of turn.",
            &["Instant"],
            Some("{1}{G}"),
        );
        assert_eq!(
            detect_roles(&heroic_intervention),
            vec!["protection".to_string()]
        );
    }

    #[test]
    fn detects_ramp_from_commander_color_identity_any_color() {
        let arcane_signet = card(
            "Arcane Signet",
            "{T}: Add one mana of any color in your commander's color identity.",
            &["Artifact"],
            Some("{2}"),
        );
        assert!(detect_roles(&arcane_signet).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_ramp_from_add_x_mana_of_any_one_color() {
        let kami = card(
            "Kami of Whispered Hopes",
            "{T}: Add X mana of any one color, where X is this creature's power.",
            &["Creature"],
            Some("{2}{G}"),
        );
        assert!(detect_roles(&kami).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_ramp_from_add_amount_equal_to() {
        let marwyn = card(
            "Marwyn, the Nurturer",
            "{T}: Add an amount of {G} equal to Marwyn's power.",
            &["Creature"],
            Some("{1}{G}"),
        );
        assert!(detect_roles(&marwyn).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_ramp_from_add_x_mana_any_combination() {
        let selvala = card(
            "Selvala, Heart of the Wilds",
            "Whenever another creature enters, you may reveal the top card of your library. If it's a land card, put it into your hand. Otherwise, add X mana in any combination of colors, where X is that card's mana value.",
            &["Creature"],
            Some("{2}{G}"),
        );
        assert!(detect_roles(&selvala).contains(&"ramp".to_string()));
    }

    #[test]
    fn detects_pioche_from_draw_cards_equal_to() {
        let greater_good = card(
            "Greater Good",
            "Sacrifice a creature: Draw cards equal to that creature's power, then discard two cards.",
            &["Enchantment"],
            None,
        );
        assert!(detect_roles(&greater_good).contains(&"pioche".to_string()));
    }

    #[test]
    fn detects_pioche_from_draw_x_cards() {
        let disciple = card(
            "Disciple of Freyalise",
            "{T}, Sacrifice a Saproling: You gain 1 life and draw X cards, where X is the number of Saprolings sacrificed this way.",
            &["Creature"],
            Some("{2}{G}"),
        );
        assert!(detect_roles(&disciple).contains(&"pioche".to_string()));
    }

    #[test]
    fn detects_pioche_from_may_draw_that_many_cards() {
        let terrasymbiosis = card(
            "Terrasymbiosis",
            "Look at the top three cards of your library. Put any number of land cards from among them onto the battlefield tapped and the rest into your hand, or you may draw that many cards.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&terrasymbiosis).contains(&"pioche".to_string()));
    }

    #[test]
    fn detects_pioche_from_draw_additional_cards() {
        let sylvan_library = card(
            "Sylvan Library",
            "At the beginning of your draw step, draw two additional cards.",
            &["Enchantment"],
            None,
        );
        assert!(detect_roles(&sylvan_library).contains(&"pioche".to_string()));
    }

    #[test]
    fn detects_removal_from_destroy_up_to_two_target() {
        let force_of_vigor = card(
            "Force of Vigor",
            "Destroy up to two target artifacts and/or enchantments.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&force_of_vigor).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn detects_removal_from_destroy_x_target() {
        let immoral_bargain = card(
            "Immoral Bargain",
            "Destroy X target nonland permanents.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&immoral_bargain).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn detects_wipe_from_deals_damage_to_each_creature() {
        let blasphemous_act = card(
            "Blasphemous Act",
            "This spell costs {1} less to cast for each creature on the battlefield. Blasphemous Act deals 13 damage to each creature.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&blasphemous_act).contains(&"wipe".to_string()));
    }

    #[test]
    fn detects_removal_from_shuffle_into_library() {
        let chaos_warp = card(
            "Chaos Warp",
            "The owner of target permanent shuffles it into their library, then reveals the top card of that library. If it's a permanent card, that player puts it onto the battlefield.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&chaos_warp).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn detects_removal_from_put_on_bottom_of_owners_library() {
        let condemn = card(
            "Condemn",
            "Put target attacking creature on the bottom of its owner's library. Its controller gains life equal to its toughness.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&condemn).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn detects_removal_from_damage_to_any_target() {
        let court_of_ire = card(
            "Court of Ire",
            "Whenever you attack, Court of Ire deals 2 damage to any target. Ferocious — If you attacked with three or more creatures this turn, it deals 7 damage to that player or planeswalker instead.",
            &["Enchantment"],
            None,
        );
        assert!(detect_roles(&court_of_ire).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn detects_ramp_from_up_to_two_basic_land_cards() {
        let cultivate = card(
            "Cultivate",
            "Search your library for up to two basic land cards, reveal them, put one onto the battlefield tapped and the other into your hand, then shuffle.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&cultivate).contains(&"ramp".to_string()));

        let kodamas_reach = card(
            "Kodama's Reach",
            "Search your library for up to two basic land cards, reveal those cards, then put one onto the battlefield tapped and the other into your hand. Then shuffle.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&kodamas_reach).contains(&"ramp".to_string()));
    }

    #[test]
    fn graveyard_hate_is_not_removal_or_wipe() {
        let crook_of_condemnation = card(
            "Crook of Condemnation",
            "{T}: Exile target card from a graveyard. If it was a creature card, you gain 1 life. {2}, {T}, Sacrifice Crook of Condemnation: Exile all graveyards.",
            &["Artifact"],
            None,
        );
        let roles = detect_roles(&crook_of_condemnation);
        assert!(!roles.contains(&"removal_cible".to_string()));
        assert!(!roles.contains(&"wipe".to_string()));

        let lantern_of_the_lost = card(
            "Lantern of the Lost",
            "{T}, Sacrifice Lantern of the Lost: Exile target card from a graveyard. {3}, {T}, Sacrifice Lantern of the Lost: Exile all cards from all graveyards.",
            &["Artifact"],
            None,
        );
        let roles = detect_roles(&lantern_of_the_lost);
        assert!(!roles.contains(&"removal_cible".to_string()));
        assert!(!roles.contains(&"wipe".to_string()));

        let dino_dna = card(
            "Dino DNA",
            "When Dino DNA enters the battlefield, exile target creature card from a graveyard. Create a token that's a copy of it, except it's a 5/5 Dinosaur in addition to its other types.",
            &["Artifact"],
            None,
        );
        assert!(!detect_roles(&dino_dna).contains(&"removal_cible".to_string()));

        let conversion_chamber = card(
            "Conversion Chamber",
            "{2}, {T}: Exile target artifact card from a graveyard. Create a 1/1 colorless Servo artifact creature token.",
            &["Artifact"],
            None,
        );
        assert!(!detect_roles(&conversion_chamber).contains(&"removal_cible".to_string()));

        let canoptek_scarab_swarm = card(
            "Canoptek Scarab Swarm",
            "{4}{B}: Exile target player's graveyard.",
            &["Creature"],
            None,
        );
        assert!(!detect_roles(&canoptek_scarab_swarm).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn land_destruction_alone_is_not_removal_cible() {
        let rumbling_crescendo = card(
            "Rumbling Crescendo",
            "Destroy up to two target lands.",
            &["Sorcery"],
            None,
        );
        assert!(!detect_roles(&rumbling_crescendo).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn removal_and_wipe_non_regression() {
        let swords = card(
            "Swords to Plowshares",
            "Exile target creature. Its controller gains life equal to its power.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&swords).contains(&"removal_cible".to_string()));

        let beast_within = card(
            "Beast Within",
            "Destroy target permanent. Its controller creates a 3/3 green Beast creature token.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&beast_within).contains(&"removal_cible".to_string()));

        let wrath = card(
            "Wrath of God",
            "Destroy all creatures. They can't be regenerated.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&wrath).contains(&"wipe".to_string()));

        let rampant_growth = card(
            "Rampant Growth",
            "Search your library for a basic land card and put it onto the battlefield tapped.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&rampant_growth).contains(&"ramp".to_string()));

        let farseek = card(
            "Farseek",
            "Search your library for an Island, Swamp, Mountain, or Plains card and put it onto the battlefield tapped. Then shuffle.",
            &["Sorcery"],
            None,
        );
        assert!(detect_roles(&farseek).contains(&"ramp".to_string()));

        let force_of_vigor = card(
            "Force of Vigor",
            "Destroy up to two target artifacts and/or enchantments.",
            &["Instant"],
            None,
        );
        assert!(detect_roles(&force_of_vigor).contains(&"removal_cible".to_string()));
    }

    #[test]
    fn nonland_card_has_no_terrain_role() {
        let vanilla = card("Grizzly Bears", "", &["Creature"], Some("{1}{G}"));
        assert!(detect_roles(&vanilla).is_empty());
    }

    #[test]
    fn a_role_correction_replaces_the_detected_roles() {
        let mut drudge_spell = card(
            "Drudge Spell",
            "Exile two creature cards from your graveyard: Create a 1/1 black Skeleton \
             creature token. When Drudge Spell leaves the battlefield, destroy all Skeleton \
             tokens.",
            &["Enchantment"],
            None,
        );
        assert!(detect_roles(&drudge_spell).contains(&"wipe".to_string()));
        drudge_spell.corrections.roles = Some(vec![]);
        assert!(detect_roles(&drudge_spell).is_empty());
        drudge_spell.corrections.roles = Some(vec!["grave_hate".to_string()]);
        assert_eq!(detect_roles(&drudge_spell), vec!["grave_hate".to_string()]);
    }

    #[test]
    fn a_role_correction_also_replaces_the_new_roles() {
        let mut mystic_sanctuary = card(
            "Mystic Sanctuary",
            "When this land enters untapped, you may put target instant or sorcery card from your graveyard on top of your library.",
            &["Land"],
            None,
        );
        mystic_sanctuary.corrections.roles =
            Some(vec!["terrain".to_string(), "recursion".to_string()]);
        assert_eq!(
            detect_roles(&mystic_sanctuary),
            vec!["terrain".to_string(), "recursion".to_string()]
        );

        let mut counterspell = card("Counterspell", "Counter target spell.", &["Instant"], None);
        counterspell.corrections.roles = Some(vec!["protection".to_string()]);
        assert_eq!(detect_roles(&counterspell), vec!["protection".to_string()]);
    }

    #[test]
    fn land_sources_are_weighted_by_quantity() {
        let forest = ResolvedCard {
            quantity: 36,
            roles: vec![],
            themes: vec![],
            card: card("Forest", "{T}: Add {G}.", &["Basic", "Land"], None),
        };
        let commander = card(
            "Atraxa, Praetors' Voice",
            "",
            &["Creature"],
            Some("{G}{W}{U}{B}"),
        );
        let base = mana_base(&[forest], &commander);
        assert_eq!(base.land_count, 36);
        assert_eq!(base.sources_by_color.get("G"), Some(&36));
    }

    #[test]
    fn counts_color_symbols_including_hybrid() {
        let mut counts = BTreeMap::new();
        count_color_symbols("{2}{W}{W}{W/U}", &mut counts);
        assert_eq!(counts.get("W"), Some(&3));
        assert_eq!(counts.get("U"), Some(&1));
    }

    #[test]
    fn role_weaknesses_flags_below_threshold() {
        let mut role_counts = BTreeMap::new();
        role_counts.insert("ramp".to_string(), 2);
        let thresholds = Thresholds::default();
        let weaknesses = role_weaknesses(&role_counts, 30, &thresholds);
        assert!(weaknesses.iter().any(|w| w.contains("base de mana")));
        assert!(
            weaknesses
                .iter()
                .any(|w| w.contains("Ramp sous-représenté"))
        );
    }

    #[test]
    fn detects_any_color_land_sources() {
        let mut counts = BTreeMap::new();
        let command_tower = card(
            "Command Tower",
            "{T}: Add one mana of any color in your commander's color identity.",
            &["Land"],
            None,
        );
        let identity: Vec<String> = COLORS.iter().map(|c| c.to_string()).collect();
        land_color_sources(&command_tower, 1, &identity, &mut counts);
        for color in COLORS {
            assert_eq!(counts.get(color), Some(&1), "missing color {color}");
        }
    }

    #[test]
    fn command_tower_bounded_to_commander_color_identity() {
        let mut counts = BTreeMap::new();
        let command_tower = card(
            "Command Tower",
            "{T}: Add one mana of any color in your commander's color identity.",
            &["Land"],
            None,
        );
        let identity = vec!["B".to_string(), "G".to_string()];
        land_color_sources(&command_tower, 1, &identity, &mut counts);
        assert_eq!(counts.get("B"), Some(&1));
        assert_eq!(counts.get("G"), Some(&1));
        assert_eq!(counts.get("R"), None);
        assert_eq!(counts.get("U"), None);
        assert_eq!(counts.get("W"), None);
    }

    #[test]
    fn exotic_orchard_style_any_color_stays_five_colors_regardless_of_identity() {
        let mut counts = BTreeMap::new();
        let exotic_orchard = card(
            "Exotic Orchard",
            "{T}: Add one mana of any color that a land an opponent controls could produce.",
            &["Land"],
            None,
        );
        let identity = vec!["B".to_string(), "G".to_string()];
        land_color_sources(&exotic_orchard, 1, &identity, &mut counts);
        for color in COLORS {
            assert_eq!(counts.get(color), Some(&1), "missing color {color}");
        }
    }

    #[test]
    fn dual_land_with_or_text_counts_both_colors() {
        let mut counts = BTreeMap::new();
        let overgrown_tomb = card("Overgrown Tomb", "{T}: Add {B} or {G}.", &["Land"], None);
        let identity = vec!["B".to_string(), "G".to_string()];
        land_color_sources(&overgrown_tomb, 1, &identity, &mut counts);
        assert_eq!(counts.get("B"), Some(&1));
        assert_eq!(counts.get("G"), Some(&1));
        assert_eq!(counts.get("R"), None);
    }

    #[test]
    fn viridescent_bog_style_add_multi_symbol_counts_once_per_color() {
        let mut counts = BTreeMap::new();
        let viridescent_bog = card("Viridescent Bog", "{T}: Add {B}{G}.", &["Land"], None);
        let identity = vec!["B".to_string(), "G".to_string()];
        land_color_sources(&viridescent_bog, 1, &identity, &mut counts);
        assert_eq!(counts.get("B"), Some(&1));
        assert_eq!(counts.get("G"), Some(&1));
    }

    #[test]
    fn twilight_mire_style_triple_option_counts_both_colors_once() {
        let mut counts = BTreeMap::new();
        let twilight_mire = card(
            "Twilight Mire",
            "{T}: Add {B}{B}, {B}{G}, or {G}{G}.",
            &["Land"],
            None,
        );
        let identity = vec!["B".to_string(), "G".to_string()];
        land_color_sources(&twilight_mire, 1, &identity, &mut counts);
        assert_eq!(counts.get("B"), Some(&1));
        assert_eq!(counts.get("G"), Some(&1));
        assert_eq!(counts.get("R"), None);
    }

    #[test]
    fn mana_base_bicolor_deck_has_no_off_color_sources_and_correct_dual_land_counts() {
        let overgrown_tomb = ResolvedCard {
            quantity: 1,
            roles: vec![],
            themes: vec![],
            card: card("Overgrown Tomb", "{T}: Add {B} or {G}.", &["Land"], None),
        };
        let forest = ResolvedCard {
            quantity: 10,
            roles: vec![],
            themes: vec![],
            card: card("Forest", "{T}: Add {G}.", &["Basic", "Land"], None),
        };
        let swamp = ResolvedCard {
            quantity: 10,
            roles: vec![],
            themes: vec![],
            card: card("Swamp", "{T}: Add {B}.", &["Basic", "Land"], None),
        };
        let mut commander = card("Dina, Essence Brewer", "", &["Creature"], Some("{2}{B}{G}"));
        commander.color_identity = vec!["B".to_string(), "G".to_string()];
        let base = mana_base(&[overgrown_tomb, forest, swamp], &commander);
        assert_eq!(base.sources_by_color.get("B"), Some(&11));
        assert_eq!(base.sources_by_color.get("G"), Some(&11));
        assert_eq!(base.sources_by_color.get("R"), None);
        assert_eq!(base.sources_by_color.get("U"), None);
        assert_eq!(base.sources_by_color.get("W"), None);
    }

    #[test]
    fn curve_weaknesses_flags_high_average_mana_value() {
        let curve = ManaCurve {
            buckets: vec![ManaCurveBucket {
                mana_value: 6,
                count: 10,
            }],
            average_mana_value: 6.0,
        };
        let weaknesses = curve_weaknesses(&curve, &Thresholds::default());
        assert!(weaknesses.iter().any(|w| w.contains("mana value moyenne")));
        assert!(weaknesses.iter().any(|w| w.contains("cartes à 6+")));
    }

    #[test]
    fn curve_weaknesses_empty_for_balanced_curve() {
        let curve = ManaCurve {
            buckets: vec![
                ManaCurveBucket {
                    mana_value: 2,
                    count: 20,
                },
                ManaCurveBucket {
                    mana_value: 3,
                    count: 15,
                },
            ],
            average_mana_value: 2.5,
        };
        let weaknesses = curve_weaknesses(&curve, &Thresholds::default());
        assert!(weaknesses.is_empty(), "{weaknesses:?}");
    }

    #[test]
    fn role_weaknesses_empty_when_above_thresholds() {
        let mut role_counts = BTreeMap::new();
        role_counts.insert("ramp".to_string(), 10);
        role_counts.insert("pioche".to_string(), 8);
        role_counts.insert("removal_cible".to_string(), 8);
        role_counts.insert("wipe".to_string(), 2);
        let weaknesses = role_weaknesses(&role_counts, 35, &Thresholds::default());
        assert!(weaknesses.is_empty(), "{weaknesses:?}");
    }
}
