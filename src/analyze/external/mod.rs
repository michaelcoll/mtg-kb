//! Sources externes interrogées par `kb analyze` : EDHREC et Recommander.

pub mod cache;
pub mod edhrec;
pub mod recommander;

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::db::cards::CardsDb;
use crate::model::{
    AnalyzeResult, Card, EdhrecRecommendation, RecommanderRecommendation, SourceError,
};

pub const MAX_RECOMMENDATIONS_PER_SOURCE: usize = 30;

pub const EDHREC_CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Default, Clone)]
pub struct ExternalSourcesResult {
    pub edhrec_recommendations: Vec<EdhrecRecommendation>,
    pub edhrec_unresolved_names: Vec<String>,
    pub recommander_recommendations: Vec<RecommanderRecommendation>,
    pub recommander_unresolved_names: Vec<String>,
    pub source_errors: Vec<SourceError>,
}

/// Interroge EDHREC puis Recommander et fusionne leurs Recommandations
/// externes filtrées. L'échec d'une Source est capturé dans `source_errors`
/// sans bloquer l'autre.
pub fn fetch_all(
    db: &CardsDb,
    analysis: &AnalyzeResult,
    edhrec_client: &dyn edhrec::EdhrecClient,
    recommander_client: &dyn recommander::RecommanderClient,
    edhrec_cache_dir: &Path,
) -> ExternalSourcesResult {
    let deck_names = deck_names_including_commander(analysis);
    let mut result = ExternalSourcesResult::default();

    match edhrec::fetch_and_filter(
        edhrec_client,
        edhrec_cache_dir,
        EDHREC_CACHE_TTL,
        analysis,
        db,
        &deck_names,
    ) {
        Ok((recommendations, unresolved_names)) => {
            result.edhrec_recommendations = recommendations;
            result.edhrec_unresolved_names = unresolved_names;
        }
        Err(e) => result.source_errors.push(SourceError {
            source: "edhrec".to_string(),
            message: e.to_string(),
        }),
    }

    match recommander::fetch_and_filter(recommander_client, analysis, db, &deck_names) {
        Ok((recommendations, unresolved_names)) => {
            result.recommander_recommendations = recommendations;
            result.recommander_unresolved_names = unresolved_names;
        }
        Err(e) => result.source_errors.push(SourceError {
            source: "recommander".to_string(),
            message: e.to_string(),
        }),
    }

    result
}

fn deck_names_including_commander(analysis: &AnalyzeResult) -> HashSet<String> {
    let mut names: HashSet<String> = analysis.cards.iter().map(|c| c.card.name.clone()).collect();
    names.insert(analysis.commander.name.clone());
    names
}

type ResolvedAndUnresolved<M> = (Vec<(Card, M)>, Vec<String>);

/// Garde, dans l'ordre d'entrée et au plus `MAX_RECOMMENDATIONS_PER_SOURCE`,
/// les Cartes résolues, légales en Commander, dans l'Identité de couleur et
/// absentes du Deck ; les noms non résolus sont listés à part.
pub(super) fn resolve_and_filter<M>(
    items: Vec<(String, M)>,
    db: &CardsDb,
    color_identity: &[String],
    deck_names: &HashSet<String>,
) -> Result<ResolvedAndUnresolved<M>> {
    let mut resolved = Vec::new();
    let mut unresolved_names = Vec::new();

    for (name, meta) in items {
        if resolved.len() >= MAX_RECOMMENDATIONS_PER_SOURCE {
            break;
        }
        let Some(card) = db.card_by_name(&name)? else {
            unresolved_names.push(name);
            continue;
        };
        if deck_names.contains(&card.name) {
            continue;
        }
        if !card
            .color_identity
            .iter()
            .all(|c| color_identity.iter().any(|a| a.eq_ignore_ascii_case(c)))
        {
            continue;
        }
        if db.is_legal_commander(&card.name)? != Some(true) {
            continue;
        }
        resolved.push((card, meta));
    }

    Ok((resolved, unresolved_names))
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

            INSERT INTO cards VALUES ('rampant', 'Rampant Growth', '{1}{G}', 2.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'text', 'G', 'G', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('bolt', 'Lightning Bolt', '{R}', 1.0, 'Instant', 'Instant',
                NULL, NULL, 'text', 'R', 'R', NULL, NULL, NULL, NULL);
            INSERT INTO cards VALUES ('elves', 'Llanowar Elves', '{G}', 1.0, 'Creature', 'Creature',
                'Elf', NULL, 'text', 'G', 'G', NULL, '1', '1', NULL);
            INSERT INTO cards VALUES ('channel', 'Channel', '{G}', 1.0, 'Sorcery', 'Sorcery',
                NULL, NULL, 'text', 'G', 'G', NULL, NULL, NULL, NULL);

            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            INSERT INTO cardLegalities VALUES ('bolt', 'Legal');
            INSERT INTO cardLegalities VALUES ('elves', 'Legal');
            INSERT INTO cardLegalities VALUES ('channel', 'Banned');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    #[test]
    fn keeps_legal_in_identity_absent_cards() {
        let (_dir, db) = fixture_db();
        let (resolved, unresolved) = resolve_and_filter(
            vec![("Rampant Growth".to_string(), 1.0)],
            &db,
            &["G".to_string()],
            &HashSet::new(),
        )
        .unwrap();
        assert_eq!(resolved.len(), 1);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn filters_out_of_color_identity() {
        let (_dir, db) = fixture_db();
        let (resolved, _) = resolve_and_filter(
            vec![("Lightning Bolt".to_string(), 1.0)],
            &db,
            &["G".to_string()],
            &HashSet::new(),
        )
        .unwrap();
        assert!(resolved.is_empty(), "Lightning Bolt is red, outside G");
    }

    #[test]
    fn filters_out_illegal_cards() {
        let (_dir, db) = fixture_db();
        let (resolved, _) = resolve_and_filter(
            vec![("Channel".to_string(), 1.0)],
            &db,
            &["G".to_string()],
            &HashSet::new(),
        )
        .unwrap();
        assert!(resolved.is_empty(), "Channel is banned in Commander");
    }

    #[test]
    fn filters_out_cards_already_in_the_deck() {
        let (_dir, db) = fixture_db();
        let mut deck_names = HashSet::new();
        deck_names.insert("Llanowar Elves".to_string());
        let (resolved, _) = resolve_and_filter(
            vec![("Llanowar Elves".to_string(), 1.0)],
            &db,
            &["G".to_string()],
            &deck_names,
        )
        .unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn lists_unresolved_names_separately() {
        let (_dir, db) = fixture_db();
        let (resolved, unresolved) = resolve_and_filter(
            vec![("Not A Real Card".to_string(), 1.0)],
            &db,
            &["G".to_string()],
            &HashSet::new(),
        )
        .unwrap();
        assert!(resolved.is_empty());
        assert_eq!(unresolved, vec!["Not A Real Card".to_string()]);
    }

    #[test]
    fn caps_at_thirty_kept_recommendations() {
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
            "#,
        )
        .unwrap();
        let mut items = Vec::new();
        for i in 0..40 {
            let uuid = format!("card-{i}");
            let name = format!("Test Card {i}");
            conn.execute(
                "INSERT INTO cards VALUES (?1, ?2, '{G}', 1.0, 'Creature', 'Creature', NULL, NULL, \
                 'text', 'G', 'G', NULL, '1', '1', NULL)",
                rusqlite::params![uuid, name],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO cardLegalities VALUES (?1, 'Legal')",
                rusqlite::params![uuid],
            )
            .unwrap();
            items.push((name, i as f64));
        }
        let db = CardsDb::open(&path).unwrap();
        let (resolved, _) =
            resolve_and_filter(items, &db, &["G".to_string()], &HashSet::new()).unwrap();
        assert_eq!(resolved.len(), MAX_RECOMMENDATIONS_PER_SOURCE);
    }

    struct FailingEdhrecClient;
    impl edhrec::EdhrecClient for FailingEdhrecClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            anyhow::bail!("réseau indisponible")
        }
    }

    struct FailingRecommanderClient;
    impl recommander::RecommanderClient for FailingRecommanderClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            anyhow::bail!("HTTP 429")
        }
    }

    struct StubEdhrecClient(String);
    impl edhrec::EdhrecClient for StubEdhrecClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    struct StubRecommanderClient(String);
    impl recommander::RecommanderClient for StubRecommanderClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    fn sample_analysis() -> AnalyzeResult {
        use crate::model::*;
        AnalyzeResult {
            commander: Card {
                name: "Test Commander".to_string(),
                mana_cost: None,
                mana_value: None,
                type_line: None,
                types: vec![],
                subtypes: vec![],
                supertypes: vec![],
                oracle_text: None,
                color_identity: vec!["G".to_string()],
                colors: vec![],
                keywords: vec![],
                power: None,
                toughness: None,
                loyalty: None,
            },
            cards: vec![],
            unresolved: vec![],
            card_count: 100,
            construction_errors: vec![],
            mana_curve: ManaCurve {
                buckets: vec![],
                average_mana_value: 0.0,
            },
            mana_base: ManaBase {
                land_count: 37,
                sources_by_color: Default::default(),
                symbols_by_color: Default::default(),
            },
            role_counts: Default::default(),
            weaknesses: vec![],
            synergies: vec![],
            candidates: vec![],
            edhrec_recommendations: vec![],
            edhrec_unresolved_names: vec![],
            recommander_recommendations: vec![],
            recommander_unresolved_names: vec![],
            source_errors: vec![],
        }
    }

    #[test]
    fn a_failing_source_produces_a_source_error_without_blocking_the_other() {
        let (_dir, db) = fixture_db();
        let recommander_json = serde_json::json!({
            "data": {"recommendations": [{"oracle_id": "x", "name": "Rampant Growth", "score": 5.0}]}
        })
        .to_string();

        let result = fetch_all(
            &db,
            &sample_analysis(),
            &FailingEdhrecClient,
            &StubRecommanderClient(recommander_json),
            dir_for_test().path(),
        );

        assert_eq!(result.source_errors.len(), 1);
        assert_eq!(result.source_errors[0].source, "edhrec");
        assert_eq!(result.recommander_recommendations.len(), 1);
    }

    #[test]
    fn both_sources_failing_produce_two_source_errors() {
        let (_dir, db) = fixture_db();
        let result = fetch_all(
            &db,
            &sample_analysis(),
            &FailingEdhrecClient,
            &FailingRecommanderClient,
            dir_for_test().path(),
        );
        assert_eq!(result.source_errors.len(), 2);
        assert!(result.edhrec_recommendations.is_empty());
        assert!(result.recommander_recommendations.is_empty());
    }

    #[test]
    fn malformed_json_produces_a_source_error() {
        let (_dir, db) = fixture_db();
        let result = fetch_all(
            &db,
            &sample_analysis(),
            &StubEdhrecClient("not json".to_string()),
            &FailingRecommanderClient,
            dir_for_test().path(),
        );
        assert_eq!(result.source_errors.len(), 2);
        assert_eq!(result.source_errors[0].source, "edhrec");
    }

    fn dir_for_test() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}
