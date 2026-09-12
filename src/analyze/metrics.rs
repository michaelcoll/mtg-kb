use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Card, ManaBase, ManaCurve, ManaCurveBucket, ResolvedCard};

const COLORS: [&str; 5] = ["W", "U", "B", "R", "G"];

pub fn is_land(card: &Card) -> bool {
    card.types.iter().any(|t| t == "Land")
}

/// Rôles détectés par motifs sur le texte oracle, la ligne de type et les
/// Keywords. Une Carte peut porter 0..n Rôles.
pub fn detect_roles(card: &Card) -> Vec<String> {
    static RAMP_LAND_SEARCH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)search your library for a .*land").unwrap());
    static MANA_ABILITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)add \{").unwrap());
    static DRAW: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)draws? (a|an|one|two|three|four|five|\d+) cards?").unwrap()
    });
    static TARGETED_REMOVAL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(destroy target|exile target|target creature gets -\d)").unwrap()
    });
    static WIPE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(destroy all|exile all|each creature (gets|deals)|all creatures get)")
            .unwrap()
    });
    static PROTECTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(hexproof|indestructible|protection from|counter target spell)").unwrap()
    });

    let mut roles = Vec::new();
    let text = card.oracle_text.as_deref().unwrap_or("");

    if is_land(card) {
        roles.push("terrain".to_string());
    }
    if !is_land(card) && (RAMP_LAND_SEARCH.is_match(text) || MANA_ABILITY.is_match(text)) {
        roles.push("ramp".to_string());
    }
    if DRAW.is_match(text) {
        roles.push("pioche".to_string());
    }
    if TARGETED_REMOVAL.is_match(text) {
        roles.push("removal_cible".to_string());
    }
    if WIPE.is_match(text) {
        roles.push("wipe".to_string());
    }
    if PROTECTION.is_match(text)
        || card
            .keywords
            .iter()
            .any(|k| k == "Hexproof" || k == "Indestructible")
    {
        roles.push("protection".to_string());
    }
    roles
}

/// Courbe de mana (hors terrains) : histogramme par mana value entière
/// (0..6, "7" regroupant 7 et plus) et mana value moyenne, pondérées par
/// quantité.
pub fn mana_curve(cards: &[ResolvedCard]) -> ManaCurve {
    let mut counts: BTreeMap<u32, u32> = BTreeMap::new();
    let mut weighted_total = 0.0;
    let mut nonland_count = 0u32;

    for resolved in cards {
        if is_land(&resolved.card) {
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

/// Symboles de couleur d'un coût de mana (ex. "{2}{W}{W}" -> W: 2), les
/// symboles hybrides comptant pour chacune de leurs couleurs (ex.
/// "{W/U}" compte pour W et pour U).
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

/// Sources de mana coloré parmi les terrains : détecte "Add {X}" dans le
/// texte oracle d'un terrain.
fn land_color_sources(card: &Card, quantity: u32, counts: &mut BTreeMap<String, u32>) {
    static ADD_SYMBOL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[Aa]dd \{([^}]+)\}").unwrap());
    let text = card.oracle_text.as_deref().unwrap_or("");
    // Une même Carte peut mentionner plusieurs fois "Add {X}" (une par
    // symbole alternatif) : on ne compte chaque couleur qu'une fois par
    // Impression, multiplié par la quantité en Deck.
    let mut colors_seen = std::collections::HashSet::new();
    for caps in ADD_SYMBOL.captures_iter(text) {
        let symbol = caps.get(1).unwrap().as_str();
        for color in COLORS {
            if symbol.contains(color) {
                colors_seen.insert(color);
            }
        }
    }
    for color in colors_seen {
        *counts.entry(color.to_string()).or_insert(0) += quantity;
    }
}

/// Base de mana : nombre de terrains, sources par couleur (parmi les
/// terrains), et symboles de couleur demandés par le reste du Deck (Cartes
/// non-terrain + Commandant).
pub fn mana_base(cards: &[ResolvedCard], commander: &Card) -> ManaBase {
    let mut land_count = 0u32;
    let mut sources_by_color: BTreeMap<String, u32> = BTreeMap::new();
    let mut symbols_by_color: BTreeMap<String, u32> = BTreeMap::new();

    for resolved in cards {
        if is_land(&resolved.card) {
            land_count += resolved.quantity;
            land_color_sources(&resolved.card, resolved.quantity, &mut sources_by_color);
        } else if let Some(cost) = &resolved.card.mana_cost {
            count_color_symbols(cost, &mut symbols_by_color);
        }
    }
    if let Some(cost) = &commander.mana_cost {
        count_color_symbols(cost, &mut symbols_by_color);
    }

    ManaBase {
        land_count,
        sources_by_color,
        symbols_by_color,
    }
}

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub min_lands: u32,
    pub min_ramp: u32,
    pub min_draw: u32,
    pub min_removal: u32,
    pub min_wipe: u32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            min_lands: 35,
            min_ramp: 10,
            min_draw: 8,
            min_removal: 8,
            min_wipe: 2,
        }
    }
}

/// Points faibles liés aux Rôles sous-représentés et à une base de mana
/// insuffisante, par comparaison à des seuils configurables.
pub fn role_weaknesses(
    role_counts: &BTreeMap<String, u32>,
    land_count: u32,
    thresholds: &Thresholds,
) -> Vec<String> {
    let mut weaknesses = Vec::new();
    let count_of = |role: &str| *role_counts.get(role).unwrap_or(&0);

    if land_count < thresholds.min_lands {
        weaknesses.push(format!(
            "base de mana insuffisante : {land_count} terrains (< {})",
            thresholds.min_lands
        ));
    }
    for (role, label, min) in [
        ("ramp", "Ramp", thresholds.min_ramp),
        ("pioche", "Pioche", thresholds.min_draw),
        ("removal_cible", "Removal ciblé", thresholds.min_removal),
        ("wipe", "Wipe", thresholds.min_wipe),
    ] {
        let count = count_of(role);
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

    fn card(name: &str, oracle_text: &str, types: &[&str], mana_cost: Option<&str>) -> Card {
        Card {
            name: name.to_string(),
            mana_cost: mana_cost.map(str::to_string),
            mana_value: None,
            type_line: None,
            types: types.iter().map(|t| t.to_string()).collect(),
            subtypes: vec![],
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
    fn nonland_card_has_no_terrain_role() {
        let vanilla = card("Grizzly Bears", "", &["Creature"], Some("{1}{G}"));
        assert!(detect_roles(&vanilla).is_empty());
    }

    #[test]
    fn land_sources_are_weighted_by_quantity() {
        let forest = ResolvedCard {
            quantity: 36,
            roles: vec![],
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
