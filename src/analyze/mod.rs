pub mod candidates;
pub mod external;
pub mod metrics;
pub mod ranking;
pub mod themes;

use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, bail};

use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::decklist::parser::{self, DecklistLine};
use crate::model::{AnalyzeResult, ExternalSources, ResolvedCard, Synergy, UnresolvedLine};

const REQUIRED_DECK_SIZE: u32 = 100;

const MAJOR_THEME_MIN_CARDS: u32 = 8;

/// Erreur uniquement si le Commandant est ambigu (0 ou 2+) : Cartes non
/// résolues et écarts de validation sont reportés dans le résultat.
pub fn run(input: &str, db: &CardsDb, thresholds: &metrics::Thresholds) -> Result<AnalyzeResult> {
    let decklist = parser::parse(input);

    if decklist.commander.is_empty() {
        bail!("aucun Commandant trouvé dans la section Commander");
    }
    let aggregated_commander = aggregate(decklist.commander);
    if aggregated_commander.len() != 1 || aggregated_commander[0].quantity != 1 {
        bail!(
            "un Deck Commander doit avoir exactement un Commandant (trouvé : {})",
            aggregated_commander
                .iter()
                .map(|l| l.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let commander_name = &aggregated_commander[0].name;
    let Some(commander) = db.card(commander_name)? else {
        bail!("Commandant introuvable dans la Base cartes : « {commander_name} »");
    };

    let mut cards = Vec::new();
    let mut unresolved = Vec::new();
    let mut construction_errors = Vec::new();
    let mut weaknesses = Vec::new();

    for line in aggregate(decklist.deck) {
        match db.card(&line.name)? {
            Some(card) => cards.push(ResolvedCard {
                quantity: line.quantity,
                roles: metrics::detect_roles(&card),
                themes: themes::detect_themes(&card),
                card,
            }),
            None => unresolved.push(UnresolvedLine {
                quantity: line.quantity,
                name: line.name,
            }),
        }
    }

    let card_count: u32 = 1
        + cards.iter().map(|c| c.quantity).sum::<u32>()
        + unresolved.iter().map(|u| u.quantity).sum::<u32>();
    if card_count != REQUIRED_DECK_SIZE {
        construction_errors.push(format!(
            "le Deck contient {card_count} cartes, {REQUIRED_DECK_SIZE} attendues"
        ));
    }

    let deck = DeckContext::new(&commander, cards.iter().map(|c| &c.card));
    for resolved in &cards {
        if resolved.quantity > 1 && !resolved.card.is_basic_land() {
            construction_errors.push(format!(
                "« {} » apparaît {} fois : le Deck doit être singleton hors terrains de base",
                resolved.card.name, resolved.quantity
            ));
        }
        if !deck.is_within_identity(&resolved.card) {
            construction_errors.push(format!(
                "« {} » (Identité de couleur {:?}) dépasse l'Identité de couleur du Commandant {:?}",
                resolved.card.name, resolved.card.color_identity, commander.color_identity
            ));
        }
        if !resolved.card.legal_in_commander {
            weaknesses.push(format!(
                "« {} » n'est pas légale en Commander",
                resolved.card.name
            ));
        }
    }

    let mana_curve = metrics::mana_curve(&cards);
    let mana_base = metrics::mana_base(&cards, &commander);
    let mut role_counts: BTreeMap<String, u32> = BTreeMap::new();
    for resolved in &cards {
        for role in &resolved.roles {
            *role_counts.entry(role.clone()).or_insert(0) += resolved.quantity;
        }
    }
    weaknesses.extend(metrics::role_weaknesses(
        &role_counts,
        mana_base.land_count,
        thresholds,
    ));
    weaknesses.extend(metrics::curve_weaknesses(&mana_curve, thresholds));

    let synergies = find_synergies(&cards);

    let mut theme_counts: BTreeMap<String, u32> = BTreeMap::new();
    for resolved in &cards {
        for theme in &resolved.themes {
            *theme_counts.entry(theme.clone()).or_insert(0) += resolved.quantity;
        }
    }
    // Les Thèmes du Commandant ne comptent pas dans le seuil de Thème majeur.
    let major_themes: HashSet<String> = theme_counts
        .into_iter()
        .filter(|(_, count)| *count >= MAJOR_THEME_MIN_CARDS)
        .map(|(theme, _)| theme)
        .collect();
    let weak_roles = metrics::weak_role_names(&role_counts, thresholds);
    let candidates = candidates::find_candidates(db, &deck, &major_themes, &weak_roles)?;

    Ok(AnalyzeResult {
        commander,
        cards,
        unresolved,
        card_count,
        construction_errors,
        mana_curve,
        mana_base,
        role_counts,
        weaknesses,
        synergies,
        candidates,
        // Remplies ensuite par `commands::analyze`, hors `--offline`.
        external: ExternalSources::default(),
    })
}

fn find_synergies(cards: &[ResolvedCard]) -> Vec<Synergy> {
    let mut cards_by_theme: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for resolved in cards {
        for theme in &resolved.themes {
            cards_by_theme
                .entry(theme.clone())
                .or_default()
                .push(resolved.card.name.clone());
        }
    }
    cards_by_theme
        .into_iter()
        .filter(|(_, cards)| cards.len() >= 2)
        .map(|(theme, cards)| Synergy { theme, cards })
        .collect()
}

fn aggregate(lines: Vec<DecklistLine>) -> Vec<DecklistLine> {
    let mut merged: Vec<DecklistLine> = Vec::new();
    for line in lines {
        if let Some(existing) = merged
            .iter_mut()
            .find(|l: &&mut DecklistLine| l.name == line.name)
        {
            existing.quantity += line.quantity;
        } else {
            merged.push(line);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::db::overrides::Corrections;
    use crate::model::CardCorrections;

    fn atraxa() -> FixtureCard {
        FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
            .mana("{G}{W}{U}{B}", 4.0)
            .types("Creature")
            .subtypes("Phyrexian, Angel, Horror")
            .supertypes("Legendary")
            .identity("B, G, U, W")
    }

    fn forest() -> FixtureCard {
        FixtureCard::new("forest", "Forest")
            .types("Land")
            .subtypes("Forest")
            .supertypes("Basic")
    }

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                atraxa(),
                FixtureCard::new("solring", "Sol Ring")
                    .mana("{1}", 1.0)
                    .types("Artifact")
                    .text("{T}: Add {C}{C}."),
                forest(),
                FixtureCard::new("lotus", "Black Lotus")
                    .mana("{0}", 0.0)
                    .types("Artifact")
                    .banned(),
                FixtureCard::new("shock", "Shock")
                    .mana("{R}", 1.0)
                    .types("Instant")
                    .identity("R"),
            ])
            .build()
    }

    fn deck_of(n: u32, extra: &str) -> String {
        let mut lines = String::from("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n");
        lines.push_str(&format!("{n} Forest\n"));
        lines.push_str(extra);
        lines
    }

    #[test]
    fn errors_when_no_commander() {
        let (_dir, db) = fixture_db();
        let err = run("Deck\n1 Sol Ring\n", &db, &metrics::Thresholds::default()).unwrap_err();
        assert!(err.to_string().contains("aucun Commandant"));
    }

    #[test]
    fn errors_when_two_commanders() {
        let (_dir, db) = fixture_db();
        let input = "Commander\n1 Atraxa, Praetors' Voice\n1 Sol Ring\n\nDeck\n";
        let err = run(input, &db, &metrics::Thresholds::default()).unwrap_err();
        assert!(err.to_string().contains("exactement un Commandant"));
    }

    #[test]
    fn reports_unresolved_cards_without_failing() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Some Unknown Card\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.unresolved[0].name, "Some Unknown Card");
    }

    #[test]
    fn flags_wrong_deck_size() {
        let (_dir, db) = fixture_db();
        let input = deck_of(10, "");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("100 attendues"))
        );
    }

    #[test]
    fn allows_many_basic_lands_but_not_duplicate_nonland() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "2 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            !result
                .construction_errors
                .iter()
                .any(|p| p.contains("Forest"))
        );
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("Sol Ring"))
        );
    }

    #[test]
    fn flags_color_identity_violation() {
        let (_dir, db) = fixture_db();
        // Atraxa is WUBG; Shock is red-identity, out of Commander's colors.
        let input = deck_of(97, "1 Shock\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .construction_errors
                .iter()
                .any(|p| p.contains("Shock") && p.contains("Identité de couleur"))
        );
    }

    #[test]
    fn flags_illegal_card_as_weakness() {
        let (_dir, db) = fixture_db();
        let input = deck_of(97, "1 Black Lotus\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .weaknesses
                .iter()
                .any(|p| p.contains("Black Lotus") && p.contains("légale"))
        );
    }

    #[test]
    fn valid_100_card_deck_has_no_construction_errors() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(result.card_count, 100);
        assert!(
            result.construction_errors.is_empty(),
            "{:?}",
            result.construction_errors
        );
    }

    #[test]
    fn mostly_lands_deck_reports_ramp_and_draw_weaknesses() {
        let (_dir, db) = fixture_db();
        let input = deck_of(98, "1 Sol Ring\n");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Ramp sous-représenté"))
        );
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Pioche sous-représenté"))
        );
        assert_eq!(result.mana_base.land_count, 98);
        assert_eq!(result.role_counts.get("terrain"), Some(&98));
        assert_eq!(result.role_counts.get("ramp"), Some(&1));
    }

    fn corrections(entries: &[(&str, CardCorrections)]) -> Corrections {
        entries
            .iter()
            .map(|(name, c)| (name.to_string(), c.clone()))
            .collect()
    }

    fn roles(values: &[&str]) -> CardCorrections {
        CardCorrections {
            roles: Some(values.iter().map(|v| v.to_string()).collect()),
            ..Default::default()
        }
    }

    /// Deck vert : deux Sorts sans Rôle détecté, un faux wipe dans le Deck
    /// et un faux wipe dans le pool des Candidats.
    fn fixture_db_for_corrections() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                atraxa(),
                forest(),
                FixtureCard::new("crippling", "Crippling Fear")
                    .types("Sorcery")
                    .text("Choose a creature type. Creatures that aren't of the chosen type get -3/-3 until end of turn.")
                    .identity("B"),
                FixtureCard::new("swarmyard", "Swarmyard Massacre")
                    .types("Sorcery")
                    .text("Creatures your opponents control get -1/-1 until end of turn for each Insect you control.")
                    .identity("B"),
                FixtureCard::new("drudge", "Drudge Spell")
                    .types("Enchantment")
                    .text("When Drudge Spell leaves the battlefield, destroy all Skeleton tokens.")
                    .identity("B"),
                FixtureCard::new("saproling", "Saproling Burst")
                    .types("Enchantment")
                    .text("When Saproling Burst leaves the battlefield, destroy all tokens created with it.")
                    .identity("G"),
            ])
            .build()
    }

    fn corrected_deck() -> String {
        deck_of(
            96,
            "1 Crippling Fear\n1 Swarmyard Massacre\n1 Drudge Spell\n",
        )
    }

    #[test]
    fn role_corrections_feed_role_counts_and_weaknesses() {
        let (_dir, db) = fixture_db_for_corrections();
        let result = run(&corrected_deck(), &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(
            result.role_counts.get("wipe"),
            Some(&1),
            "detected: Drudge Spell"
        );
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Wipe sous-représenté"))
        );

        let db = db.with_corrections(corrections(&[
            ("Crippling Fear", roles(&["wipe"])),
            ("Swarmyard Massacre", roles(&["wipe", "grave_hate"])),
            ("Drudge Spell", roles(&[])),
        ]));
        let result = run(&corrected_deck(), &db, &metrics::Thresholds::default()).unwrap();
        assert_eq!(result.role_counts.get("wipe"), Some(&2));
        assert_eq!(result.role_counts.get("grave_hate"), Some(&1));
        assert!(
            !result
                .weaknesses
                .iter()
                .any(|w| w.contains("Wipe sous-représenté"))
        );
        assert!(
            !result
                .weaknesses
                .iter()
                .any(|w| w.to_lowercase().contains("grave")),
            "a role without threshold never yields a weakness: {:?}",
            result.weaknesses
        );
        let drudge = result
            .cards
            .iter()
            .find(|c| c.card.name == "Drudge Spell")
            .unwrap();
        assert!(drudge.roles.is_empty());
        let json = serde_json::to_value(drudge).unwrap();
        assert_eq!(json["card"]["overridden"], serde_json::json!(["roles"]));
    }

    #[test]
    fn role_corrections_apply_to_candidates() {
        let (_dir, db) = fixture_db_for_corrections();
        let deck = deck_of(98, "1 Crippling Fear\n");
        let result = run(&deck, &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .candidates
                .iter()
                .any(|c| c.card.name == "Saproling Burst")
        );

        let db = db.with_corrections(corrections(&[
            ("Saproling Burst", roles(&[])),
            ("Swarmyard Massacre", roles(&["wipe"])),
        ]));
        let result = run(&deck, &db, &metrics::Thresholds::default()).unwrap();
        let names: Vec<_> = result
            .candidates
            .iter()
            .map(|c| c.card.name.as_str())
            .collect();
        assert!(!names.contains(&"Saproling Burst"), "{names:?}");
        assert!(names.contains(&"Swarmyard Massacre"), "{names:?}");
    }

    #[test]
    fn a_banning_correction_flags_the_deck_card_and_drops_the_candidate() {
        let (_dir, db) = fixture_db_for_corrections();
        let banned = CardCorrections {
            legal_in_commander: Some(false),
            ..Default::default()
        };
        let db = db.with_corrections(corrections(&[
            ("Drudge Spell", banned.clone()),
            ("Saproling Burst", banned),
        ]));
        let result = run(&corrected_deck(), &db, &metrics::Thresholds::default()).unwrap();
        assert!(
            result
                .weaknesses
                .iter()
                .any(|w| w.contains("Drudge Spell") && w.contains("légale"))
        );
        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Saproling Burst")
        );
    }

    fn goblin(uuid: &str, name: &str, identity: &str) -> FixtureCard {
        FixtureCard::new(uuid, name)
            .types("Creature")
            .subtypes("Goblin")
            .identity(identity)
    }

    /// `count` Gobelins d'identité `identity`, et les lignes de Decklist qui
    /// les contiennent.
    fn goblin_grunts(count: u32, identity: &str) -> (Vec<FixtureCard>, String) {
        let cards: Vec<FixtureCard> = (0..count)
            .map(|i| {
                goblin(
                    &format!("goblin-deck-{i}"),
                    &format!("Goblin Grunt {i}"),
                    identity,
                )
            })
            .collect();
        let deck_lines = (0..count)
            .map(|i| format!("1 Goblin Grunt {i}\n"))
            .collect();
        (cards, deck_lines)
    }

    fn fixture_db_with_goblin_pool(goblins_in_deck: u32) -> (tempfile::TempDir, CardsDb, String) {
        let (grunts, deck_lines) = goblin_grunts(goblins_in_deck, "G");
        let (dir, db) = CardsFixture::new()
            .cards([
                atraxa(),
                forest(),
                goblin("goblin-pool", "Goblin Raider", "G"),
            ])
            .cards(grunts)
            .build();
        (dir, db, deck_lines)
    }

    #[test]
    fn a_minor_tribal_theme_yields_no_candidate() {
        let (_dir, db, goblin_lines) = fixture_db_with_goblin_pool(3);
        let input =
            format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n95 Forest\n{goblin_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Goblin Raider"),
            "tribal:Goblin is carried by only 3 cards, below the major-theme threshold"
        );
    }

    #[test]
    fn a_theme_carried_by_at_least_eight_cards_is_major_and_yields_candidates() {
        let (_dir, db, goblin_lines) = fixture_db_with_goblin_pool(8);
        let input =
            format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n90 Forest\n{goblin_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        let raider = result
            .candidates
            .iter()
            .find(|c| c.card.name == "Goblin Raider")
            .expect("tribal:Goblin, carried by 8 deck cards, is a major theme");
        assert_eq!(raider.matched_themes, vec!["tribal:Goblin".to_string()]);
    }

    #[test]
    fn the_commander_is_never_a_candidate() {
        let (grunts, deck_lines) = goblin_grunts(8, "R");
        let (_dir, db) = CardsFixture::new()
            .cards([
                goblin("goblin-commander", "Goblin Warlord", "R").supertypes("Legendary"),
                forest(),
            ])
            .cards(grunts)
            .build();
        let input = format!("Commander\n1 Goblin Warlord\n\nDeck\n91 Forest\n{deck_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Goblin Warlord"),
            "the Commander matches the major tribal:Goblin theme but is already in the Deck"
        );
    }

    #[test]
    fn the_commander_does_not_count_towards_the_major_theme_threshold() {
        // 7 Gobelins + le Commandant Gobelin : 8 si le Commandant était compté.
        let (grunts, deck_lines) = goblin_grunts(7, "R");
        let (_dir, db) = CardsFixture::new()
            .cards([
                goblin("goblin-commander", "Goblin Warlord", "R").supertypes("Legendary"),
                forest(),
                goblin("goblin-pool", "Goblin Raider", "R"),
            ])
            .cards(grunts)
            .build();
        let input = format!("Commander\n1 Goblin Warlord\n\nDeck\n92 Forest\n{deck_lines}");
        let result = run(&input, &db, &metrics::Thresholds::default()).unwrap();

        assert!(
            !result
                .candidates
                .iter()
                .any(|c| c.card.name == "Goblin Raider"),
            "tribal:Goblin is carried by only 7 Deck cards; the Commander must not count towards \
             the 8-card major-theme threshold"
        );
    }
}
