use anyhow::Result;

use crate::analyze;
use crate::analyze::external::edhrec::HttpEdhrecClient;
use crate::analyze::external::recommander::HttpRecommanderClient;
use crate::analyze::external::{self, Clients};
use crate::analyze::metrics::Thresholds;
use crate::data_dir::edhrec_cache_dir;
use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::decklist;
use crate::model::{AnalyzeResult, Bracket};
use crate::output::print_json;

pub fn run(
    source: &str,
    thresholds: &Thresholds,
    bracket: Option<Bracket>,
    offline: bool,
) -> Result<()> {
    let input = decklist::read_source(source)?;

    let db = super::open_cards_db()?;
    let cache_dir = edhrec_cache_dir();
    let clients = Clients {
        edhrec: &HttpEdhrecClient,
        recommander: &HttpRecommanderClient,
        edhrec_cache_dir: &cache_dir,
    };
    let result = analyze_deck(&input, &db, thresholds, bracket, offline, clients)?;

    print_json(&result)
}

/// Hors ligne, `clients` n'est jamais appelé.
fn analyze_deck(
    input: &str,
    db: &CardsDb,
    thresholds: &Thresholds,
    bracket: Option<Bracket>,
    offline: bool,
    clients: Clients<'_>,
) -> Result<AnalyzeResult> {
    let mut result = analyze::run(input, db, thresholds, bracket)?;

    if !offline {
        result.external = external::fetch_all(
            db,
            &DeckContext::from_analysis(&result),
            &analyze::major_themes(&result.cards, &result.commander),
            clients,
        );
    }
    analyze::origins::assign_origins(&mut result);

    Ok(result)
}

#[cfg(test)]
mod personal_decklists;

/// Aides partagées par les tests de `kb analyze`.
#[cfg(test)]
mod test_support {
    use std::path::Path;

    use super::*;
    use crate::analyze::external::edhrec::EdhrecClient;
    use crate::analyze::external::recommander::RecommanderClient;
    use crate::analyze::external::test_clients::OfflineClient;
    use crate::db::fixture::FixtureCard;

    /// Terrain de base produisant `color`.
    pub fn basic(uuid: &str, name: &str, color: &str) -> FixtureCard {
        FixtureCard::new(uuid, name)
            .types("Land")
            .subtypes(name)
            .supertypes("Basic")
            .text(&format!("({{T}}: Add {{{color}}}.)"))
            .identity(color)
    }

    /// `kb analyze --offline`, sans Bracket : tout appel réseau échoue le test.
    pub fn analyze_offline(db: &CardsDb, input: &str, thresholds: &Thresholds) -> AnalyzeResult {
        try_analyze_offline(db, input, thresholds).unwrap()
    }

    pub fn try_analyze_offline(
        db: &CardsDb,
        input: &str,
        thresholds: &Thresholds,
    ) -> Result<AnalyzeResult> {
        let cache_dir = tempfile::tempdir().unwrap();
        analyze_deck(
            input,
            db,
            thresholds,
            None,
            true,
            Clients {
                edhrec: &OfflineClient,
                recommander: &OfflineClient,
                edhrec_cache_dir: cache_dir.path(),
            },
        )
    }

    /// `kb analyze` en ligne, avec les seuils par défaut.
    pub fn analyze_online(
        db: &CardsDb,
        input: &str,
        bracket: Option<Bracket>,
        edhrec: &dyn EdhrecClient,
        recommander: &dyn RecommanderClient,
        edhrec_cache_dir: &Path,
    ) -> AnalyzeResult {
        analyze_deck(
            input,
            db,
            &Thresholds::default(),
            bracket,
            false,
            Clients {
                edhrec,
                recommander,
                edhrec_cache_dir,
            },
        )
        .unwrap()
    }

    /// Candidats terrains (Rôle faible « terrain »), dans l'ordre.
    pub fn land_candidate_names(result: &AnalyzeResult) -> Vec<&str> {
        result
            .candidates
            .iter()
            .filter(|c| c.matched_weak_roles.iter().any(|r| r == "terrain"))
            .map(|c| c.card.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{analyze_offline, analyze_online};
    use super::*;
    use crate::analyze::external::test_clients::{
        FailingClient, StubEdhrecClient, StubRecommanderClient,
    };
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::model::Origin;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W"),
                FixtureCard::new("forest", "Forest")
                    .types("Land")
                    .supertypes("Basic"),
                FixtureCard::new("rampant", "Rampant Growth")
                    .mana("{1}{G}", 2.0)
                    .types("Sorcery")
                    .text("Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.")
                    .identity("G"),
                FixtureCard::new("cultivate", "Cultivate")
                    .mana("{2}{G}", 3.0)
                    .types("Sorcery")
                    .identity("G"),
            ])
            .build()
    }

    fn deck_input() -> String {
        "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n99 Forest\n".to_string()
    }

    #[test]
    fn offline_never_calls_external_clients_and_leaves_lists_empty() {
        let (_dir, db) = fixture_db();
        let result = analyze_offline(&db, &deck_input(), &Thresholds::default());

        assert!(result.external.edhrec_recommendations.is_empty());
        assert!(result.external.recommander_recommendations.is_empty());
        assert!(result.external.source_errors.is_empty());
    }

    /// Les Sources externes restent des clés de premier niveau du JSON, dans
    /// le même ordre qu'avant leur regroupement dans `ExternalSources`.
    #[test]
    fn json_keys_are_flat_and_in_the_documented_order() {
        let (_dir, db) = fixture_db();
        let result = analyze_offline(&db, &deck_input(), &Thresholds::default());

        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(
            top_level_keys_in_order(&json),
            vec![
                "commander",
                "cards",
                "unresolved",
                "card_count",
                "bracket",
                "construction_errors",
                "mana_curve",
                "mana_base",
                "role_counts",
                "thresholds",
                "weaknesses",
                "synergies",
                "candidates",
                "edhrec_recommendations",
                "edhrec_unresolved_names",
                "recommander_recommendations",
                "recommander_unresolved_names",
                "source_errors",
            ]
        );
        let round_trip: AnalyzeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, result);
    }

    /// `serde_json::Value` trie les clés : on lit l'objet en flux pour garder
    /// l'ordre d'écriture.
    fn top_level_keys_in_order(json: &str) -> Vec<String> {
        struct Keys(Vec<String>);
        impl<'de> serde::Deserialize<'de> for Keys {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct Visitor;
                impl<'de> serde::de::Visitor<'de> for Visitor {
                    type Value = Keys;
                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("un objet JSON")
                    }
                    fn visit_map<A: serde::de::MapAccess<'de>>(
                        self,
                        mut map: A,
                    ) -> Result<Keys, A::Error> {
                        let mut keys = Vec::new();
                        while let Some(key) = map.next_key::<String>()? {
                            map.next_value::<serde::de::IgnoredAny>()?;
                            keys.push(key);
                        }
                        Ok(Keys(keys))
                    }
                }
                d.deserialize_map(Visitor)
            }
        }
        serde_json::from_str::<Keys>(json).unwrap().0
    }

    #[test]
    fn a_json_without_external_source_keys_still_parses() {
        let (_dir, db) = fixture_db();
        let result = analyze::run(&deck_input(), &db, &Thresholds::default(), None).unwrap();
        let mut json = serde_json::to_value(&result).unwrap();
        let object = json.as_object_mut().unwrap();
        for key in [
            "edhrec_recommendations",
            "edhrec_unresolved_names",
            "recommander_recommendations",
            "recommander_unresolved_names",
            "source_errors",
        ] {
            object.remove(key);
        }
        let parsed: AnalyzeResult = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, result);
    }

    #[test]
    fn a_json_without_thresholds_reads_back_the_default_thresholds() {
        let (_dir, db) = fixture_db();
        let custom = Thresholds {
            min_ramp: 3,
            ..Thresholds::default()
        };
        let result = analyze::run(&deck_input(), &db, &custom, None).unwrap();
        let mut json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["thresholds"]["min_ramp"], 3);
        json.as_object_mut().unwrap().remove("thresholds");

        let parsed: AnalyzeResult = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.thresholds, Thresholds::default());
    }

    #[test]
    fn online_merges_external_recommendations_into_the_result() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let edhrec_json = serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": [
                    {"name": "Rampant Growth", "synergy": 0.2, "num_decks": 1, "potential_decks": 2}
                ]}
            ]}}
        })
        .to_string();
        let recommander_json = serde_json::json!({
            "data": {"recommendations": [{"name": "Rampant Growth", "score": 4.0}]}
        })
        .to_string();

        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(recommander_json),
            cache_dir.path(),
        );

        assert_eq!(result.external.edhrec_recommendations.len(), 1);
        assert_eq!(result.external.recommander_recommendations.len(), 1);
        assert!(result.external.source_errors.is_empty());
    }

    fn signals_fixture_db() -> (tempfile::TempDir, CardsDb) {
        let bala_ged = |side: &str, face: &str, types: &str| {
            FixtureCard::new(
                &format!("bala-ged-{side}"),
                "Bala Ged Recovery // Bala Ged Sanctuary",
            )
            .face("modal_dfc", side, face, 3.0)
            .mana_value(3.0)
            .types(types)
            .identity("G")
            .edhrec_rank(812)
            .salt(0.4)
        };
        CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W")
                    .edhrec_rank(40)
                    .salt(1.9),
                FixtureCard::new("forest", "Forest")
                    .types("Land")
                    .supertypes("Basic"),
                FixtureCard::new("rampant", "Rampant Growth")
                    .mana("{1}{G}", 2.0)
                    .types("Sorcery")
                    .text("Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.")
                    .identity("G")
                    .edhrec_rank(150)
                    .salt(0.2)
                    .game_changer(),
                bala_ged("a", "Bala Ged Recovery", "Sorcery"),
                bala_ged("b", "Bala Ged Sanctuary", "Land"),
            ])
            .build()
    }

    fn signals(card: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "edhrec_rank": card["edhrec_rank"],
            "salt": card["salt"],
            "game_changer": card["game_changer"],
        })
    }

    #[test]
    fn every_card_in_the_json_carries_its_quality_signals() {
        let (_dir, db) = signals_fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let edhrec_json = serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": [
                    {"name": "Rampant Growth", "synergy": 0.2, "num_decks": 1, "potential_decks": 2}
                ]}
            ]}}
        })
        .to_string();
        let input =
            "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n1 Bala Ged Recovery\n98 Forest\n";

        let result = analyze_online(
            &db,
            input,
            None,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(r#"{"data": {"recommendations": []}}"#.to_string()),
            cache_dir.path(),
        );
        let json = serde_json::to_value(&result).unwrap();

        assert_eq!(
            signals(&json["commander"]),
            serde_json::json!({"edhrec_rank": 40, "salt": 1.9, "game_changer": false})
        );
        let deck_card = |name: &str| {
            json["cards"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["card"]["name"] == name)
                .unwrap()["card"]
                .clone()
        };
        assert_eq!(
            signals(&deck_card("Bala Ged Recovery // Bala Ged Sanctuary")),
            serde_json::json!({"edhrec_rank": 812, "salt": 0.4, "game_changer": false})
        );
        assert_eq!(
            signals(&deck_card("Forest")),
            serde_json::json!({"edhrec_rank": null, "salt": null, "game_changer": false}),
            "une valeur absente est explicitement nulle"
        );
        let deck_card_json = deck_card("Forest");
        assert!(deck_card_json.get("edhrec_rank").is_some());
        assert!(deck_card_json.get("salt").is_some());

        let rampant = serde_json::json!({"edhrec_rank": 150, "salt": 0.2, "game_changer": true});
        let candidate = json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["card"]["name"] == "Rampant Growth")
            .expect("Rampant Growth est Candidat (ramp sous-représenté)");
        assert_eq!(signals(&candidate["card"]), rampant);
        assert_eq!(signals(&json["edhrec_recommendations"][0]["card"]), rampant);
    }

    #[test]
    fn a_card_json_without_quality_signals_still_parses() {
        let (_dir, db) = signals_fixture_db();
        let result = analyze::run(
            "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n99 Forest\n",
            &db,
            &Thresholds::default(),
            None,
        )
        .unwrap();
        let mut json = serde_json::to_value(&result).unwrap();
        let commander = json["commander"].as_object_mut().unwrap();
        for key in ["edhrec_rank", "salt", "game_changer"] {
            commander.remove(key);
        }

        let parsed: AnalyzeResult = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.commander.edhrec_rank, None);
        assert_eq!(parsed.commander.salt, None);
        assert!(!parsed.commander.game_changer);
    }

    fn edhrec_json_with(names: &[&str]) -> String {
        let cardviews: Vec<_> = names
            .iter()
            .map(|n| serde_json::json!({"name": n, "synergy": 0.2, "num_decks": 1, "potential_decks": 2}))
            .collect();
        serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": cardviews}
            ]}}
        })
        .to_string()
    }

    fn recommander_json_with(names: &[&str]) -> String {
        let recommendations: Vec<_> = names
            .iter()
            .map(|n| serde_json::json!({"name": n, "score": 4.0}))
            .collect();
        serde_json::json!({"data": {"recommendations": recommendations}}).to_string()
    }

    /// Origines par nom de Carte, triées par nom (l'ordre des listes n'est
    /// pas l'objet de ces tests).
    fn origins_by_name<'a>(
        cards: impl Iterator<Item = (&'a str, &'a [Origin])>,
    ) -> Vec<(&'a str, Vec<Origin>)> {
        let mut by_name: Vec<_> = cards.map(|(n, o)| (n, o.to_vec())).collect();
        by_name.sort_by_key(|(n, _)| *n);
        by_name
    }

    #[test]
    fn a_card_in_candidates_and_edhrec_carries_both_origins_in_both_lists() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &StubEdhrecClient(edhrec_json_with(&["Rampant Growth", "Cultivate"])),
            &StubRecommanderClient(recommander_json_with(&["Cultivate"])),
            cache_dir.path(),
        );

        let candidate = result
            .candidates
            .iter()
            .find(|c| c.card.name == "Rampant Growth")
            .expect("Rampant Growth est un Candidat ramp");
        assert_eq!(candidate.origins, vec![Origin::Kb, Origin::Edhrec]);
        assert_eq!(
            origins_by_name(
                result
                    .external
                    .edhrec_recommendations
                    .iter()
                    .map(|r| (r.card.name.as_str(), r.origins.as_slice()))
            ),
            vec![
                ("Cultivate", vec![Origin::Edhrec, Origin::Recommander]),
                ("Rampant Growth", vec![Origin::Kb, Origin::Edhrec]),
            ]
        );
        assert_eq!(
            origins_by_name(
                result
                    .external
                    .recommander_recommendations
                    .iter()
                    .map(|r| (r.card.name.as_str(), r.origins.as_slice()))
            ),
            vec![("Cultivate", vec![Origin::Edhrec, Origin::Recommander])]
        );
    }

    #[test]
    fn a_card_in_a_single_list_carries_a_single_origin() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &StubEdhrecClient(edhrec_json_with(&["Cultivate"])),
            &StubRecommanderClient(recommander_json_with(&[])),
            cache_dir.path(),
        );

        assert_eq!(
            origins_by_name(
                result
                    .candidates
                    .iter()
                    .map(|c| (c.card.name.as_str(), c.origins.as_slice()))
            ),
            vec![("Rampant Growth", vec![Origin::Kb])]
        );
        assert_eq!(
            origins_by_name(
                result
                    .external
                    .edhrec_recommendations
                    .iter()
                    .map(|r| (r.card.name.as_str(), r.origins.as_slice()))
            ),
            vec![("Cultivate", vec![Origin::Edhrec])]
        );
    }

    #[test]
    fn offline_candidates_only_have_origin_kb() {
        let (_dir, db) = fixture_db();
        let result = analyze_offline(&db, &deck_input(), &Thresholds::default());

        assert!(!result.candidates.is_empty());
        assert!(result.candidates.iter().all(|c| c.origins == [Origin::Kb]));
    }

    #[test]
    fn a_json_without_origins_still_parses() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &StubEdhrecClient(edhrec_json_with(&["Cultivate"])),
            &StubRecommanderClient(recommander_json_with(&["Cultivate"])),
            cache_dir.path(),
        );
        let mut json = serde_json::to_value(&result).unwrap();
        for list in [
            "candidates",
            "edhrec_recommendations",
            "recommander_recommendations",
        ] {
            for item in json[list].as_array_mut().unwrap() {
                item.as_object_mut().unwrap().remove("origins");
            }
        }

        let parsed: AnalyzeResult = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.candidates.len(), result.candidates.len());
        assert!(parsed.candidates.iter().all(|c| c.origins.is_empty()));
        assert!(
            parsed
                .external
                .recommander_recommendations
                .iter()
                .all(|r| r.origins.is_empty())
        );
    }

    #[test]
    fn edhrec_ties_on_synergy_are_broken_by_inclusion_rate_then_name() {
        let (_dir, db) = CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W"),
                FixtureCard::new("forest", "Forest")
                    .types("Land")
                    .supertypes("Basic"),
                FixtureCard::new("cultivate", "Cultivate").identity("G"),
                FixtureCard::new("kodama", "Kodama's Reach").identity("G"),
                FixtureCard::new("rampant", "Rampant Growth").identity("G"),
                FixtureCard::new("elves", "Llanowar Elves").identity("G"),
            ])
            .build();
        let cache_dir = tempfile::tempdir().unwrap();
        let edhrec_json = serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": [
                    {"name": "Cultivate", "synergy": 0.2, "num_decks": 1, "potential_decks": 10},
                    {"name": "Rampant Growth", "synergy": 0.2, "num_decks": 8, "potential_decks": 10},
                    {"name": "Kodama's Reach", "synergy": 0.2, "num_decks": 8, "potential_decks": 10},
                    {"name": "Llanowar Elves", "synergy": 0.5, "num_decks": 1, "potential_decks": 10}
                ]}
            ]}}
        })
        .to_string();

        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(r#"{"data": {"recommendations": []}}"#.to_string()),
            cache_dir.path(),
        );

        let names: Vec<&str> = result
            .external
            .edhrec_recommendations
            .iter()
            .map(|r| r.card.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec![
                "Llanowar Elves",
                "Kodama's Reach",
                "Rampant Growth",
                "Cultivate"
            ]
        );
    }

    #[test]
    fn a_failing_source_is_reported_without_failing_the_whole_analysis() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_online(
            &db,
            &deck_input(),
            None,
            &FailingClient,
            &FailingClient,
            cache_dir.path(),
        );

        assert_eq!(result.external.source_errors.len(), 2);
        assert!(result.external.edhrec_recommendations.is_empty());
        assert!(result.external.recommander_recommendations.is_empty());
    }

    /// Les Rôles tutor, recursion, contresort et grave_hate sont comptés et
    /// exposés par Carte, sans seuil : ni Point faible, ni panier de Candidats.
    #[test]
    fn roles_without_threshold_are_counted_but_never_a_weakness() {
        let (_dir, db) = CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W"),
                FixtureCard::new("forest", "Forest")
                    .types("Land")
                    .supertypes("Basic"),
                FixtureCard::new("demonic", "Demonic Tutor")
                    .mana("{1}{B}", 2.0)
                    .types("Sorcery")
                    .identity("B")
                    .text("Search your library for a card, put that card into your hand, then shuffle."),
                FixtureCard::new("regrowth", "Regrowth")
                    .mana("{1}{G}", 2.0)
                    .types("Sorcery")
                    .identity("G")
                    .text("Return target card from your graveyard to your hand."),
                FixtureCard::new("counterspell", "Counterspell")
                    .mana("{U}{U}", 2.0)
                    .types("Instant")
                    .identity("U")
                    .text("Counter target spell."),
                FixtureCard::new("rip", "Rest in Peace")
                    .mana("{1}{W}", 2.0)
                    .types("Enchantment")
                    .identity("W")
                    .text("When Rest in Peace enters, exile all graveyards."),
                FixtureCard::new("mystical", "Mystical Tutor")
                    .mana("{U}", 1.0)
                    .types("Instant")
                    .identity("U")
                    .text("Search your library for an instant or sorcery card, reveal it, then shuffle and put that card on top."),
            ])
            .build();
        let input = "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n\
                     1 Demonic Tutor\n1 Regrowth\n1 Counterspell\n1 Rest in Peace\n95 Forest\n";
        let result = analyze_offline(&db, input, &Thresholds::default());

        for role in ["tutor", "recursion", "contresort", "grave_hate"] {
            assert_eq!(result.role_counts.get(role), Some(&1), "{role}");
        }
        assert_eq!(result.role_counts.get("protection"), None);
        let counterspell = result
            .cards
            .iter()
            .find(|c| c.card.name == "Counterspell")
            .unwrap();
        assert_eq!(counterspell.roles, vec!["contresort".to_string()]);
        for weakness in &result.weaknesses {
            let weakness = weakness.to_lowercase();
            for role in ["tutor", "recursion", "contresort", "grave"] {
                assert!(!weakness.contains(role), "{weakness}");
            }
        }
        assert!(
            result
                .candidates
                .iter()
                .all(|c| c.card.name != "Mystical Tutor"),
            "un Rôle sans seuil ne produit pas de panier de Candidats"
        );
    }
}

/// Pages EDHREC par Thème majeur (#97).
#[cfg(test)]
mod theme_pages_tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::Path;

    use super::*;
    use crate::analyze::external::edhrec::EdhrecClient;
    use crate::analyze::external::test_clients::StubRecommanderClient;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    /// Client EDHREC bouchonné : sert le JSON de chaque page connue,
    /// échoue sur les autres, et enregistre les pages demandées.
    #[derive(Default)]
    struct PagesClient {
        pages: HashMap<String, String>,
        requested: RefCell<Vec<String>>,
    }

    impl PagesClient {
        fn page(mut self, page: &str, names: &[&str]) -> Self {
            self.pages.insert(page.to_string(), edhrec_json(names));
            self
        }

        fn requested(&self) -> Vec<String> {
            self.requested.borrow().clone()
        }
    }

    impl EdhrecClient for PagesClient {
        fn fetch(&self, page: &str) -> Result<String> {
            self.requested.borrow_mut().push(page.to_string());
            match self.pages.get(page) {
                Some(json) => Ok(json.clone()),
                None => anyhow::bail!("HTTP 404 ({page})"),
            }
        }
    }

    fn empty_recommander() -> StubRecommanderClient {
        StubRecommanderClient(r#"{"data": {"recommendations": []}}"#.to_string())
    }

    fn edhrec_json(names: &[&str]) -> String {
        let cardviews: Vec<_> = names
            .iter()
            .map(|n| serde_json::json!({"name": n, "synergy": 0.2, "num_decks": 1, "potential_decks": 2}))
            .collect();
        serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": cardviews}
            ]}}
        })
        .to_string()
    }

    /// Le texte de Krenko porte les Thèmes `tokens` et `tribal:Goblin` (dans
    /// la table), majeurs d'office ; `tribal:Homunculus`, de sa seule ligne de
    /// type, ne l'est pas.
    fn fixture_db(extra: impl IntoIterator<Item = FixtureCard>) -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("krenko", "Krenko, Mob Boss")
                    .types("Creature")
                    .subtypes("Goblin, Homunculus")
                    .supertypes("Legendary")
                    .text("{T}: Create X 1/1 red Goblin creature tokens, where X is the number of Goblins you control.")
                    .identity("R"),
                FixtureCard::new("mountain", "Mountain")
                    .types("Land")
                    .subtypes("Mountain")
                    .supertypes("Basic")
                    .identity("R"),
                FixtureCard::new("bolt", "Lightning Bolt")
                    .types("Instant")
                    .identity("R"),
                FixtureCard::new("tremors", "Impact Tremors")
                    .types("Enchantment")
                    .identity("R"),
                FixtureCard::new("instigator", "Goblin Instigator")
                    .types("Creature")
                    .identity("R"),
            ])
            .cards(extra)
            .build()
    }

    const DECK: &str = "Commander\n1 Krenko, Mob Boss\n\nDeck\n99 Mountain\n";

    fn analyze_online(db: &CardsDb, client: &PagesClient, cache_dir: &Path) -> AnalyzeResult {
        super::test_support::analyze_online(db, DECK, None, client, &empty_recommander(), cache_dir)
    }

    #[test]
    fn each_major_theme_in_the_table_queries_its_page_and_others_none() {
        let (_dir, db) = fixture_db([]);
        let cache_dir = tempfile::tempdir().unwrap();
        let client = PagesClient::default();

        analyze_online(&db, &client, cache_dir.path());

        assert_eq!(
            client.requested(),
            vec![
                "krenko-mob-boss",
                "krenko-mob-boss/tokens",
                "krenko-mob-boss/goblins",
            ]
        );
    }

    fn edhrec_themes(result: &AnalyzeResult) -> Vec<(&str, Option<&str>)> {
        result
            .external
            .edhrec_recommendations
            .iter()
            .map(|r| (r.card.name.as_str(), r.theme.as_deref()))
            .collect()
    }

    #[test]
    fn theme_page_recommendations_carry_their_theme_and_commander_page_ones_none() {
        let (_dir, db) = fixture_db([]);
        let cache_dir = tempfile::tempdir().unwrap();
        let client = PagesClient::default()
            .page("krenko-mob-boss", &["Lightning Bolt"])
            .page("krenko-mob-boss/tokens", &["Impact Tremors"])
            .page("krenko-mob-boss/goblins", &["Goblin Instigator"]);

        let result = analyze_online(&db, &client, cache_dir.path());

        assert_eq!(
            edhrec_themes(&result),
            vec![
                ("Lightning Bolt", None),
                ("Impact Tremors", Some("tokens")),
                ("Goblin Instigator", Some("tribal:Goblin")),
            ]
        );
        let json = serde_json::to_value(&result).unwrap();
        let recommendations = json["edhrec_recommendations"].as_array().unwrap();
        assert!(recommendations[0].get("theme").is_none());
        assert_eq!(recommendations[1]["theme"], "tokens");
    }

    #[test]
    fn the_cap_of_thirty_applies_per_page() {
        let names =
            |prefix: &str| -> Vec<String> { (0..35).map(|i| format!("{prefix} {i:02}")).collect() };
        let (commander_names, tokens_names) = (names("Burn"), names("Token Maker"));
        let (_dir, db) = fixture_db(
            commander_names
                .iter()
                .chain(&tokens_names)
                .map(|n| FixtureCard::new(n, n).types("Instant").identity("R")),
        );
        let cache_dir = tempfile::tempdir().unwrap();
        fn as_strs(v: &[String]) -> Vec<&str> {
            v.iter().map(String::as_str).collect()
        }
        let client = PagesClient::default()
            .page("krenko-mob-boss", &as_strs(&commander_names))
            .page("krenko-mob-boss/tokens", &as_strs(&tokens_names));

        let result = analyze_online(&db, &client, cache_dir.path());

        let themes = edhrec_themes(&result);
        assert_eq!(themes.iter().filter(|(_, t)| t.is_none()).count(), 30);
        assert_eq!(
            themes.iter().filter(|(_, t)| *t == Some("tokens")).count(),
            30
        );
    }

    #[test]
    fn a_failing_theme_page_is_a_source_error_naming_the_theme_and_keeps_other_pages() {
        let (_dir, db) = fixture_db([]);
        let cache_dir = tempfile::tempdir().unwrap();
        let client = PagesClient::default()
            .page("krenko-mob-boss", &["Lightning Bolt"])
            .page("krenko-mob-boss/goblins", &["Goblin Instigator"]);

        let result = analyze_online(&db, &client, cache_dir.path());

        let sources: Vec<&str> = result
            .external
            .source_errors
            .iter()
            .map(|e| e.source.as_str())
            .collect();
        assert_eq!(sources, vec!["edhrec:tokens"]);
        assert_eq!(
            edhrec_themes(&result),
            vec![
                ("Lightning Bolt", None),
                ("Goblin Instigator", Some("tribal:Goblin")),
            ]
        );
    }

    #[test]
    fn a_theme_page_is_served_from_the_cache_within_seven_days() {
        let (_dir, db) = fixture_db([]);
        let cache_dir = tempfile::tempdir().unwrap();
        let first = PagesClient::default()
            .page("krenko-mob-boss", &["Lightning Bolt"])
            .page("krenko-mob-boss/tokens", &["Impact Tremors"])
            .page("krenko-mob-boss/goblins", &["Goblin Instigator"]);
        let first_result = analyze_online(&db, &first, cache_dir.path());

        let unreachable = PagesClient::default();
        let second_result = analyze_online(&db, &unreachable, cache_dir.path());

        assert!(unreachable.requested().is_empty());
        assert_eq!(edhrec_themes(&second_result), edhrec_themes(&first_result));
    }

    #[test]
    fn offline_queries_no_theme_page() {
        let (_dir, db) = fixture_db([]);
        let cache_dir = tempfile::tempdir().unwrap();
        let client = PagesClient::default();

        let result = analyze_deck(
            DECK,
            &db,
            &Thresholds::default(),
            None,
            true,
            Clients {
                edhrec: &client,
                recommander: &empty_recommander(),
                edhrec_cache_dir: cache_dir.path(),
            },
        )
        .unwrap();

        assert!(client.requested().is_empty());
        assert!(result.external.edhrec_recommendations.is_empty());
    }
}

/// Terrains Candidats quand la base de mana est insuffisante (#90).
#[cfg(test)]
mod land_candidates_tests {
    use super::test_support::{basic, land_candidate_names};
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    fn dual(uuid: &str, name: &str, identity: &str) -> FixtureCard {
        FixtureCard::new(uuid, name)
            .types("Land")
            .text("{T}: Add {G} or {U}.")
            .identity(identity)
    }

    /// Atraxa, deux terrains de base, un dual dans l'Identité, un dual hors
    /// Identité et `extra_duals` terrains non-base supplémentaires.
    fn fixture_db(extra_duals: usize) -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W"),
                basic("forest", "Forest", "G"),
                basic("plains", "Plains", "W"),
                dual("pool", "Breeding Pool", "G, U"),
                FixtureCard::new("crypt", "Blood Crypt")
                    .types("Land")
                    .text("{T}: Add {B} or {R}.")
                    .identity("B, R"),
            ])
            .cards(
                (0..extra_duals)
                    .map(|i| dual(&format!("dual-{i}"), &format!("Simic Land {i:02}"), "G, U")),
            )
            .build()
    }

    fn analyze_offline(db: &CardsDb, forests: u32) -> AnalyzeResult {
        let input = format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n{forests} Forest\n");
        super::test_support::analyze_offline(db, &input, &Thresholds::default())
    }

    #[test]
    fn a_deck_below_min_lands_gets_eligible_nonbasic_land_candidates() {
        let (_dir, db) = fixture_db(0);
        let result = analyze_offline(&db, 30);

        let names = land_candidate_names(&result);
        assert_eq!(names, vec!["Breeding Pool"], "{:?}", result.candidates);
    }

    #[test]
    fn basic_lands_are_never_candidates() {
        let (_dir, db) = fixture_db(0);
        let result = analyze_offline(&db, 30);

        let names: Vec<_> = result
            .candidates
            .iter()
            .map(|c| c.card.name.as_str())
            .collect();
        assert!(!names.contains(&"Plains"), "{names:?}");
        assert!(!names.contains(&"Forest"), "{names:?}");
    }

    #[test]
    fn a_deck_at_min_lands_gets_no_land_candidates() {
        let (_dir, db) = fixture_db(0);
        let result = analyze_offline(&db, Thresholds::default().min_lands);

        assert!(land_candidate_names(&result).is_empty());
    }

    #[test]
    fn land_candidates_are_capped_at_ten_without_duplicates() {
        let (_dir, db) = fixture_db(15);
        let result = analyze_offline(&db, 30);

        assert_eq!(land_candidate_names(&result).len(), 10);
        let unique: std::collections::HashSet<&str> = result
            .candidates
            .iter()
            .map(|c| c.card.name.as_str())
            .collect();
        assert_eq!(unique.len(), result.candidates.len());
    }
}

/// Bracket et limite de Game Changers (#96).
#[cfg(test)]
mod bracket_tests {
    use super::*;
    use crate::analyze::external::test_clients::{StubEdhrecClient, StubRecommanderClient};
    use crate::db::fixture::{CardsFixture, FixtureCard};

    const DECK_GAME_CHANGERS: [&str; 3] = ["Demonic Tutor", "Cyclonic Rift", "Smothering Tithe"];

    /// Atraxa, Forest, deux ramps Candidats (Mana Crypt, Game Changer, et
    /// Rampant Growth) et trois Game Changers à mettre dans le Deck.
    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("atraxa", "Atraxa, Praetors' Voice")
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("B, G, U, W"),
                FixtureCard::new("forest", "Forest")
                    .types("Land")
                    .supertypes("Basic"),
                FixtureCard::new("crypt", "Mana Crypt")
                    .types("Artifact")
                    .text("{T}: Add {C}{C}.")
                    .game_changer(),
                FixtureCard::new("rampant", "Rampant Growth")
                    .types("Sorcery")
                    .text("Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.")
                    .identity("G"),
                FixtureCard::new("tutor", "Demonic Tutor")
                    .types("Sorcery")
                    .identity("B")
                    .game_changer(),
                FixtureCard::new("rift", "Cyclonic Rift")
                    .types("Instant")
                    .identity("U")
                    .game_changer(),
                FixtureCard::new("tithe", "Smothering Tithe")
                    .types("Enchantment")
                    .identity("W")
                    .game_changer(),
            ])
            .build()
    }

    /// Deck de 100 cartes avec les `game_changers` premiers Game Changers.
    fn deck_with_game_changers(game_changers: usize) -> String {
        let mut input = format!(
            "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n{} Forest\n",
            99 - game_changers
        );
        for name in &DECK_GAME_CHANGERS[..game_changers] {
            input.push_str(&format!("1 {name}\n"));
        }
        input
    }

    fn recommending_both_ramps() -> (StubEdhrecClient, StubRecommanderClient) {
        let edhrec = serde_json::json!({
            "container": {"json_dict": {"cardlists": [
                {"header": "Top Cards", "cardviews": [
                    {"name": "Mana Crypt", "synergy": 0.5, "num_decks": 9, "potential_decks": 10},
                    {"name": "Rampant Growth", "synergy": 0.2, "num_decks": 5, "potential_decks": 10}
                ]}
            ]}}
        });
        let recommander = serde_json::json!({
            "data": {"recommendations": [
                {"name": "Mana Crypt", "score": 9.0},
                {"name": "Rampant Growth", "score": 4.0}
            ]}
        });
        (
            StubEdhrecClient(edhrec.to_string()),
            StubRecommanderClient(recommander.to_string()),
        )
    }

    fn analyze_in(bracket: Option<u8>, game_changers_in_deck: usize) -> AnalyzeResult {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let (edhrec, recommander) = recommending_both_ramps();
        super::test_support::analyze_online(
            &db,
            &deck_with_game_changers(game_changers_in_deck),
            bracket.map(|b| Bracket::try_from(b).unwrap()),
            &edhrec,
            &recommander,
            cache_dir.path(),
        )
    }

    /// Noms proposés, toutes listes confondues : Candidats, EDHREC, Recommander.
    fn proposed(result: &AnalyzeResult) -> Vec<&str> {
        result
            .candidates
            .iter()
            .map(|c| &c.card)
            .chain(
                result
                    .external
                    .edhrec_recommendations
                    .iter()
                    .map(|r| &r.card),
            )
            .chain(
                result
                    .external
                    .recommander_recommendations
                    .iter()
                    .map(|r| &r.card),
            )
            .map(|card| card.name.as_str())
            .collect()
    }

    fn is_offered_everywhere(result: &AnalyzeResult, name: &str) -> bool {
        result.candidates.iter().any(|c| c.card.name == name)
            && result
                .external
                .edhrec_recommendations
                .iter()
                .any(|r| r.card.name == name)
            && result
                .external
                .recommander_recommendations
                .iter()
                .any(|r| r.card.name == name)
    }

    #[test]
    fn in_bracket_2_no_game_changer_is_proposed() {
        let result = analyze_in(Some(2), 0);

        assert!(
            !proposed(&result).contains(&"Mana Crypt"),
            "{:?}",
            proposed(&result)
        );
        assert!(is_offered_everywhere(&result, "Rampant Growth"));
    }

    #[test]
    fn in_bracket_3_game_changers_are_proposed_only_below_three_in_the_deck() {
        let with_two = analyze_in(Some(3), 2);
        assert!(is_offered_everywhere(&with_two, "Mana Crypt"));

        let with_three = analyze_in(Some(3), 3);
        assert!(
            !proposed(&with_three).contains(&"Mana Crypt"),
            "{:?}",
            proposed(&with_three)
        );
        assert!(is_offered_everywhere(&with_three, "Rampant Growth"));
    }

    #[test]
    fn in_bracket_4_or_without_bracket_nothing_is_filtered() {
        for bracket in [Some(4), Some(5), None] {
            let result = analyze_in(bracket, 3);
            assert!(is_offered_everywhere(&result, "Mana Crypt"), "{bracket:?}");
            assert!(
                !result.weaknesses.iter().any(|w| w.contains("Game Changer")),
                "{bracket:?} : {:?}",
                result.weaknesses
            );
        }
    }

    #[test]
    fn the_bracket_is_written_in_the_json() {
        let json = serde_json::to_value(analyze_in(Some(3), 0)).unwrap();
        assert_eq!(json["bracket"], 3);

        let json = serde_json::to_value(analyze_in(None, 0)).unwrap();
        assert_eq!(json["bracket"], serde_json::Value::Null);
    }

    #[test]
    fn a_json_without_bracket_still_parses() {
        let mut json = serde_json::to_value(analyze_in(Some(3), 0)).unwrap();
        json.as_object_mut().unwrap().remove("bracket");
        let parsed: AnalyzeResult = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.bracket, None);
    }

    #[test]
    fn a_deck_over_the_bracket_limit_has_a_weakness_naming_its_game_changers() {
        let result = analyze_in(Some(2), 1);
        let weakness = result
            .weaknesses
            .iter()
            .find(|w| w.contains("Game Changer"))
            .unwrap_or_else(|| panic!("{:?}", result.weaknesses));
        assert!(weakness.contains("Demonic Tutor"), "{weakness}");
        assert!(weakness.contains("Bracket 2"), "{weakness}");
    }

    #[test]
    fn a_deck_at_the_bracket_limit_has_no_game_changer_weakness() {
        let result = analyze_in(Some(3), 3);
        assert!(
            !result.weaknesses.iter().any(|w| w.contains("Game Changer")),
            "{:?}",
            result.weaknesses
        );
    }
}

/// Point faible « couleur sous-alimentée » et terrains Candidats qui la
/// corrigent (#95).
#[cfg(test)]
mod undersupplied_color_tests {
    use super::test_support::{analyze_offline, basic, land_candidate_names};
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    fn land(uuid: &str, name: &str, text: &str, identity: &str) -> FixtureCard {
        FixtureCard::new(uuid, name)
            .types("Land")
            .text(text)
            .identity(identity)
    }

    /// Commandants Simic ({G}{U}) et mono-vert ({G}), terrains de base,
    /// Wastes, et un pool de terrains non-base : vert seul, bleu seul, dual.
    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("simic", "Simic Commander")
                    .mana("{G}{U}", 2.0)
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("G, U"),
                FixtureCard::new("mono", "Green Commander")
                    .mana("{G}", 1.0)
                    .types("Creature")
                    .supertypes("Legendary")
                    .identity("G"),
                basic("forest", "Forest", "G"),
                basic("island", "Island", "U"),
                FixtureCard::new("wastes", "Wastes")
                    .types("Land")
                    .supertypes("Basic")
                    .text("({T}: Add {C}.)"),
                land("grove", "Alpha Grove", "{T}: Add {G}.", "G"),
                land("lagoon", "Zeta Lagoon", "{T}: Add {U}.", "U"),
                land("pool", "Breeding Pool", "{T}: Add {G} or {U}.", "G, U"),
            ])
            .build()
    }

    /// Deck Simic : le Commandant porte 50 % de symboles G et 50 % de U.
    fn simic_deck(forests: u32, islands: u32) -> String {
        format!("Commander\n1 Simic Commander\n\nDeck\n{forests} Forest\n{islands} Island\n")
    }

    fn color_weaknesses(result: &AnalyzeResult) -> Vec<&str> {
        result
            .weaknesses
            .iter()
            .filter(|w| w.starts_with("couleur sous-alimentée"))
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn a_color_with_30_percent_of_sources_and_50_percent_of_symbols_is_undersupplied() {
        let (_dir, db) = fixture_db();
        let result = analyze_offline(&db, &simic_deck(21, 9), &Thresholds::default());

        assert_eq!(
            color_weaknesses(&result),
            vec![
                "couleur sous-alimentée : U porte 50 % des symboles de mana mais 30 % des \
                 sources (écart > 10 points)"
            ]
        );
    }

    #[test]
    fn a_gap_equal_to_the_threshold_is_not_a_weakness() {
        let (_dir, db) = fixture_db();
        // U : 50 % des symboles, 40 % des sources.
        let result = analyze_offline(&db, &simic_deck(6, 4), &Thresholds::default());

        assert!(
            color_weaknesses(&result).is_empty(),
            "{:?}",
            result.weaknesses
        );
    }

    #[test]
    fn the_gap_threshold_is_configurable() {
        let (_dir, db) = fixture_db();
        let thresholds = Thresholds {
            max_color_source_gap: 20.0,
            ..Thresholds::default()
        };
        let result = analyze_offline(&db, &simic_deck(21, 9), &thresholds);
        assert!(
            color_weaknesses(&result).is_empty(),
            "{:?}",
            result.weaknesses
        );

        let thresholds = Thresholds {
            max_color_source_gap: 5.0,
            ..Thresholds::default()
        };
        let result = analyze_offline(&db, &simic_deck(6, 4), &thresholds);
        assert_eq!(
            color_weaknesses(&result),
            vec![
                "couleur sous-alimentée : U porte 50 % des symboles de mana mais 40 % des \
                 sources (écart > 5 points)"
            ]
        );
    }

    #[test]
    fn a_monocolor_deck_never_has_an_undersupplied_color() {
        let (_dir, db) = fixture_db();
        // G : 100 % des symboles, 1 source sur 21 terrains.
        let input = "Commander\n1 Green Commander\n\nDeck\n1 Forest\n20 Wastes\n";
        let result = analyze_offline(&db, input, &Thresholds::default());

        assert!(
            color_weaknesses(&result).is_empty(),
            "{:?}",
            result.weaknesses
        );
    }

    #[test]
    fn land_candidates_producing_an_undersupplied_color_come_first() {
        let (_dir, db) = fixture_db();
        // 30 terrains (< 35) et U sous-alimenté.
        let result = analyze_offline(&db, &simic_deck(21, 9), &Thresholds::default());

        assert_eq!(
            land_candidate_names(&result),
            vec!["Breeding Pool", "Zeta Lagoon", "Alpha Grove"]
        );
    }

    #[test]
    fn without_an_undersupplied_color_land_candidates_keep_their_usual_order() {
        let (_dir, db) = fixture_db();
        let result = analyze_offline(&db, &simic_deck(15, 15), &Thresholds::default());

        assert_eq!(
            land_candidate_names(&result),
            vec!["Alpha Grove", "Breeding Pool", "Zeta Lagoon"]
        );
    }
}
