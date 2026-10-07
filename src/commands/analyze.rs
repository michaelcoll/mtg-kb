use std::path::Path;

use anyhow::Result;

use crate::analyze;
use crate::analyze::external;
use crate::analyze::external::edhrec::{EdhrecClient, HttpEdhrecClient};
use crate::analyze::external::recommander::{HttpRecommanderClient, RecommanderClient};
use crate::analyze::metrics::Thresholds;
use crate::data_dir::edhrec_cache_dir;
use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::decklist;
use crate::model::AnalyzeResult;
use crate::output::print_json;

pub fn run(source: &str, thresholds: &Thresholds, offline: bool) -> Result<()> {
    let input = decklist::read_source(source)?;

    let db = super::open_cards_db()?;
    let result = analyze_deck(
        &input,
        &db,
        thresholds,
        offline,
        &HttpEdhrecClient,
        &HttpRecommanderClient,
        &edhrec_cache_dir(),
    )?;

    print_json(&result)
}

fn analyze_deck(
    input: &str,
    db: &CardsDb,
    thresholds: &Thresholds,
    offline: bool,
    edhrec_client: &dyn EdhrecClient,
    recommander_client: &dyn RecommanderClient,
    edhrec_cache_dir: &Path,
) -> Result<AnalyzeResult> {
    let mut result = analyze::run(input, db, thresholds)?;

    if !offline {
        result.external = external::fetch_all(
            db,
            &DeckContext::from_analysis(&result),
            edhrec_client,
            recommander_client,
            edhrec_cache_dir,
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

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
                    .identity("G"),
            ])
            .build()
    }

    fn deck_input() -> String {
        "Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n99 Forest\n".to_string()
    }

    struct PanicIfCalledEdhrecClient;
    impl EdhrecClient for PanicIfCalledEdhrecClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            panic!("--offline ne doit jamais appeler EDHREC")
        }
    }

    struct PanicIfCalledRecommanderClient;
    impl RecommanderClient for PanicIfCalledRecommanderClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            panic!("--offline ne doit jamais appeler Recommander")
        }
    }

    #[test]
    fn offline_never_calls_external_clients_and_leaves_lists_empty() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_deck(
            &deck_input(),
            &db,
            &Thresholds::default(),
            true,
            &PanicIfCalledEdhrecClient,
            &PanicIfCalledRecommanderClient,
            cache_dir.path(),
        )
        .unwrap();

        assert!(result.external.edhrec_recommendations.is_empty());
        assert!(result.external.recommander_recommendations.is_empty());
        assert!(result.external.source_errors.is_empty());
    }

    /// Les Sources externes restent des clés de premier niveau du JSON, dans
    /// le même ordre qu'avant leur regroupement dans `ExternalSources`.
    #[test]
    fn json_keys_are_flat_and_in_the_documented_order() {
        let (_dir, db) = fixture_db();
        let cache_dir = tempfile::tempdir().unwrap();
        let result = analyze_deck(
            &deck_input(),
            &db,
            &Thresholds::default(),
            true,
            &PanicIfCalledEdhrecClient,
            &PanicIfCalledRecommanderClient,
            cache_dir.path(),
        )
        .unwrap();

        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(
            top_level_keys_in_order(&json),
            vec![
                "commander",
                "cards",
                "unresolved",
                "card_count",
                "construction_errors",
                "mana_curve",
                "mana_base",
                "role_counts",
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
        let result = analyze::run(&deck_input(), &db, &Thresholds::default()).unwrap();
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

    struct StubEdhrecClient(String);
    impl EdhrecClient for StubEdhrecClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    struct StubRecommanderClient(String);
    impl RecommanderClient for StubRecommanderClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            Ok(self.0.clone())
        }
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

        let result = analyze_deck(
            &deck_input(),
            &db,
            &Thresholds::default(),
            false,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(recommander_json),
            cache_dir.path(),
        )
        .unwrap();

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

        let result = analyze_deck(
            input,
            &db,
            &Thresholds::default(),
            false,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(r#"{"data": {"recommendations": []}}"#.to_string()),
            cache_dir.path(),
        )
        .unwrap();
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

        let result = analyze_deck(
            &deck_input(),
            &db,
            &Thresholds::default(),
            false,
            &StubEdhrecClient(edhrec_json),
            &StubRecommanderClient(r#"{"data": {"recommendations": []}}"#.to_string()),
            cache_dir.path(),
        )
        .unwrap();

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

        struct FailingClient;
        impl EdhrecClient for FailingClient {
            fn fetch(&self, _slug: &str) -> Result<String> {
                anyhow::bail!("HTTP 429")
            }
        }
        impl RecommanderClient for FailingClient {
            fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
                anyhow::bail!("HTTP 429")
            }
        }

        let result = analyze_deck(
            &deck_input(),
            &db,
            &Thresholds::default(),
            false,
            &FailingClient,
            &FailingClient,
            cache_dir.path(),
        )
        .unwrap();

        assert_eq!(result.external.source_errors.len(), 2);
        assert!(result.external.edhrec_recommendations.is_empty());
        assert!(result.external.recommander_recommendations.is_empty());
    }
}

/// Terrains Candidats quand la base de mana est insuffisante (#90).
#[cfg(test)]
mod land_candidates_tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    fn basic(uuid: &str, name: &str, color: &str) -> FixtureCard {
        FixtureCard::new(uuid, name)
            .types("Land")
            .subtypes(name)
            .supertypes("Basic")
            .text(&format!("({{T}}: Add {{{color}}}.)"))
            .identity(color)
    }

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

    struct NoEdhrec;
    impl EdhrecClient for NoEdhrec {
        fn fetch(&self, _slug: &str) -> Result<String> {
            panic!("--offline ne doit jamais appeler EDHREC")
        }
    }

    struct NoRecommander;
    impl RecommanderClient for NoRecommander {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            panic!("--offline ne doit jamais appeler Recommander")
        }
    }

    fn analyze_offline(db: &CardsDb, forests: u32) -> AnalyzeResult {
        let cache_dir = tempfile::tempdir().unwrap();
        let input = format!("Commander\n1 Atraxa, Praetors' Voice\n\nDeck\n{forests} Forest\n");
        analyze_deck(
            &input,
            db,
            &Thresholds::default(),
            true,
            &NoEdhrec,
            &NoRecommander,
            cache_dir.path(),
        )
        .unwrap()
    }

    fn land_candidate_names(result: &AnalyzeResult) -> Vec<&str> {
        result
            .candidates
            .iter()
            .filter(|c| c.matched_weak_roles.iter().any(|r| r == "terrain"))
            .map(|c| c.card.name.as_str())
            .collect()
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
