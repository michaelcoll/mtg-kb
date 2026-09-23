use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

use crate::analyze;
use crate::analyze::external::edhrec::{EdhrecClient, HttpEdhrecClient};
use crate::analyze::external::recommander::{HttpRecommanderClient, RecommanderClient};
use crate::analyze::external::{self, ExternalSourcesResult};
use crate::analyze::metrics::Thresholds;
use crate::data_dir::edhrec_cache_dir;
use crate::db::cards::CardsDb;
use crate::model::AnalyzeResult;
use crate::output::print_json;

#[allow(clippy::too_many_arguments)]
pub fn run(
    source: &str,
    min_lands: u32,
    min_ramp: u32,
    min_draw: u32,
    min_removal: u32,
    min_wipe: u32,
    max_average_mana_value: f64,
    max_high_cost_cards: u32,
    offline: bool,
) -> Result<()> {
    let input = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de la Decklist depuis l'entrée standard")?;
        buf
    } else {
        std::fs::read_to_string(source).with_context(|| format!("lecture du fichier {source}"))?
    };

    let thresholds = Thresholds {
        min_lands,
        min_ramp,
        min_draw,
        min_removal,
        min_wipe,
        max_average_mana_value,
        max_high_cost_cards,
    };

    let db = super::open_cards_db()?;
    let result = analyze_deck(
        &input,
        &db,
        &thresholds,
        offline,
        &HttpEdhrecClient,
        &HttpRecommanderClient,
        &edhrec_cache_dir(),
    )?;

    print_json(&result)
}

#[allow(clippy::too_many_arguments)]
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
        let outcome = external::fetch_all(
            db,
            &result,
            edhrec_client,
            recommander_client,
            edhrec_cache_dir,
        );
        merge_external_sources(&mut result, outcome);
    }

    Ok(result)
}

fn merge_external_sources(result: &mut AnalyzeResult, outcome: ExternalSourcesResult) {
    result.edhrec_recommendations = outcome.edhrec_recommendations;
    result.edhrec_unresolved_names = outcome.edhrec_unresolved_names;
    result.recommander_recommendations = outcome.recommander_recommendations;
    result.recommander_unresolved_names = outcome.recommander_unresolved_names;
    result.source_errors = outcome.source_errors;
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

        assert!(result.edhrec_recommendations.is_empty());
        assert!(result.recommander_recommendations.is_empty());
        assert!(result.source_errors.is_empty());
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

        assert_eq!(result.edhrec_recommendations.len(), 1);
        assert_eq!(result.recommander_recommendations.len(), 1);
        assert!(result.source_errors.is_empty());
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

        assert_eq!(result.source_errors.len(), 2);
        assert!(result.edhrec_recommendations.is_empty());
        assert!(result.recommander_recommendations.is_empty());
    }
}
