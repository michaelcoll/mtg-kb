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
