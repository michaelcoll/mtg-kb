use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};

use crate::analyze;
use crate::analyze::external::edhrec::{EdhrecClient, HttpEdhrecClient};
use crate::analyze::external::recommander::{HttpRecommanderClient, RecommanderClient};
use crate::analyze::external::{self, ExternalSourcesResult};
use crate::analyze::metrics::Thresholds;
use crate::data_dir::{cards_db_path, edhrec_cache_dir};
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

    let db = CardsDb::open(&cards_db_path())?;
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

/// Calcule l'`AnalyzeResult` (voir `analyze::run`, sans dépendance réseau)
/// puis, hors `--offline`, interroge les Sources externes et fusionne leurs
/// Recommandations externes et erreurs éventuelles. Séparé de `run` pour
/// être testable avec des clients de test, sans réseau (voir ADR 0003 et
/// les critères d'acceptation de l'issue #38).
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

/// Fusionne le résultat de l'interrogation des Sources externes dans
/// l'`AnalyzeResult` : rien à faire de plus, `analyze::run` produit déjà des
/// listes vides pour ces champs (voir ADR 0003 : `analyze::run` reste sans
/// dépendance réseau).
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
    use rusqlite::Connection;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AllPrintings.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE cards (
                uuid TEXT, name TEXT, manaCost TEXT, manaValue REAL, type TEXT, types TEXT,
                subtypes TEXT, supertypes TEXT, text TEXT, colorIdentity TEXT,
                colors TEXT, keywords TEXT, power TEXT, toughness TEXT, loyalty TEXT
            );
            CREATE TABLE cardLegalities (uuid TEXT, commander TEXT);

            INSERT INTO cards VALUES ('atraxa', 'Atraxa, Praetors'' Voice', '{G}{W}{U}{B}', 4.0,
                'Legendary Creature', 'Creature', NULL, 'Legendary', 'text', 'B, G, U, W',
                'W, U, B, G', NULL, '4', '4', NULL);
            INSERT INTO cards VALUES ('forest', 'Forest', NULL, 0.0, 'Basic Land — Forest', 'Land',
                'Forest', 'Basic', 'text', NULL, NULL, NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('rampant', 'Rampant Growth', '{1}{G}', 2.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'text', 'G', 'G', NULL, NULL, NULL, NULL);

            INSERT INTO cardLegalities VALUES ('atraxa', 'Legal');
            INSERT INTO cardLegalities VALUES ('forest', 'Legal');
            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
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
