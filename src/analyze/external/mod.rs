//! Sources externes interrogées par `kb analyze` : EDHREC et Recommander.

pub mod cache;
pub mod edhrec;
pub mod recommander;

use std::path::Path;
use std::time::Duration;

use anyhow::Result;

use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::model::{Card, ExternalSources, SourceError};

pub const MAX_RECOMMENDATIONS_PER_SOURCE: usize = 30;

pub const EDHREC_CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Clients des Sources externes et cache des pages EDHREC.
#[derive(Clone, Copy)]
pub struct Clients<'a> {
    pub edhrec: &'a dyn edhrec::EdhrecClient,
    pub recommander: &'a dyn recommander::RecommanderClient,
    pub edhrec_cache_dir: &'a Path,
}

/// Interroge EDHREC (page Commandant, puis une page par Thème majeur de la
/// table) puis Recommander, et fusionne leurs Recommandations externes
/// filtrées. L'échec d'une page ou d'une Source est capturé dans
/// `source_errors` (`edhrec`, `edhrec:<Thème>`, `recommander`) sans bloquer
/// les autres.
pub fn fetch_all<'a>(
    db: &CardsDb,
    deck: &DeckContext,
    major_themes: impl IntoIterator<Item = &'a String>,
    clients: Clients<'_>,
) -> ExternalSources {
    let mut result = ExternalSources::default();

    for page in edhrec::pages(deck, major_themes) {
        match edhrec::fetch_and_filter(
            clients.edhrec,
            clients.edhrec_cache_dir,
            EDHREC_CACHE_TTL,
            db,
            deck,
            &page,
        ) {
            Ok((recommendations, unresolved_names)) => {
                result.edhrec_recommendations.extend(recommendations);
                for name in unresolved_names {
                    if !result.edhrec_unresolved_names.contains(&name) {
                        result.edhrec_unresolved_names.push(name);
                    }
                }
            }
            Err(e) => result.source_errors.push(SourceError {
                source: match &page.theme {
                    Some(theme) => format!("edhrec:{theme}"),
                    None => "edhrec".to_string(),
                },
                message: e.to_string(),
            }),
        }
    }

    match recommander::fetch_and_filter(clients.recommander, db, deck) {
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

type ResolvedAndUnresolved<M> = (Vec<(Card, M)>, Vec<String>);

/// Garde, dans l'ordre d'entrée et au plus `MAX_RECOMMENDATIONS_PER_SOURCE`,
/// les Cartes résolues et éligibles (`DeckContext`) ; les noms non résolus
/// sont listés à part.
pub(super) fn resolve_and_filter<M>(
    items: Vec<(String, M)>,
    db: &CardsDb,
    deck: &DeckContext,
) -> Result<ResolvedAndUnresolved<M>> {
    let mut resolved = Vec::new();
    let mut unresolved_names = Vec::new();

    for (name, meta) in items {
        if resolved.len() >= MAX_RECOMMENDATIONS_PER_SOURCE {
            break;
        }
        let Some(card) = db.card(&name)? else {
            unresolved_names.push(name);
            continue;
        };
        if deck.eligibility(&card).is_ok() {
            resolved.push((card, meta));
        }
    }

    Ok((resolved, unresolved_names))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("rampant", "Rampant Growth").identity("G"),
                FixtureCard::new("bolt", "Lightning Bolt").identity("R"),
                FixtureCard::new("elves", "Llanowar Elves").identity("G"),
                FixtureCard::new("channel", "Channel")
                    .identity("G")
                    .banned(),
            ])
            .build()
    }

    /// Deck d'un Commandant vert contenant les Cartes `deck_names`.
    fn green_deck(deck_names: &[&str]) -> DeckContext {
        let commander = Card::named("Test Commander", &["G"]);
        let cards: Vec<Card> = deck_names.iter().map(|n| Card::named(n, &["G"])).collect();
        DeckContext::new(&commander, &cards)
    }

    #[test]
    fn keeps_legal_in_identity_absent_cards() {
        let (_dir, db) = fixture_db();
        let (resolved, unresolved) = resolve_and_filter(
            vec![("Rampant Growth".to_string(), 1.0)],
            &db,
            &green_deck(&[]),
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
            &green_deck(&[]),
        )
        .unwrap();
        assert!(resolved.is_empty(), "Lightning Bolt is red, outside G");
    }

    #[test]
    fn filters_out_illegal_cards() {
        let (_dir, db) = fixture_db();
        let (resolved, _) =
            resolve_and_filter(vec![("Channel".to_string(), 1.0)], &db, &green_deck(&[])).unwrap();
        assert!(resolved.is_empty(), "Channel is banned in Commander");
    }

    #[test]
    fn filters_out_cards_already_in_the_deck() {
        let (_dir, db) = fixture_db();
        let (resolved, _) = resolve_and_filter(
            vec![("Llanowar Elves".to_string(), 1.0)],
            &db,
            &green_deck(&["Llanowar Elves"]),
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
            &green_deck(&[]),
        )
        .unwrap();
        assert!(resolved.is_empty());
        assert_eq!(unresolved, vec!["Not A Real Card".to_string()]);
    }

    #[test]
    fn caps_at_thirty_kept_recommendations() {
        let items: Vec<(String, f64)> = (0..40)
            .map(|i| (format!("Test Card {i}"), i as f64))
            .collect();
        let (_dir, db) = CardsFixture::new()
            .cards((0..40).map(|i| {
                FixtureCard::new(&format!("card-{i}"), &format!("Test Card {i}")).identity("G")
            }))
            .build();
        let (resolved, _) = resolve_and_filter(items, &db, &green_deck(&[])).unwrap();
        assert_eq!(resolved.len(), MAX_RECOMMENDATIONS_PER_SOURCE);
    }

    use test_clients::{FailingClient, StubEdhrecClient, StubRecommanderClient};

    fn clients<'a>(
        edhrec: &'a dyn edhrec::EdhrecClient,
        recommander: &'a dyn recommander::RecommanderClient,
        cache_dir: &'a tempfile::TempDir,
    ) -> Clients<'a> {
        Clients {
            edhrec,
            recommander,
            edhrec_cache_dir: cache_dir.path(),
        }
    }

    #[test]
    fn a_failing_source_produces_a_source_error_without_blocking_the_other() {
        let (_dir, db) = fixture_db();
        let recommander_json = serde_json::json!({
            "data": {"recommendations": [{"oracle_id": "x", "name": "Rampant Growth", "score": 5.0}]}
        })
        .to_string();

        let cache_dir = dir_for_test();
        let result = fetch_all(
            &db,
            &green_deck(&[]),
            [],
            clients(
                &FailingClient,
                &StubRecommanderClient(recommander_json),
                &cache_dir,
            ),
        );

        assert_eq!(result.source_errors.len(), 1);
        assert_eq!(result.source_errors[0].source, "edhrec");
        assert_eq!(result.recommander_recommendations.len(), 1);
    }

    #[test]
    fn the_commander_is_never_an_external_recommendation() {
        let (_dir, db) = CardsFixture::new()
            .cards([
                FixtureCard::new("commander", "Test Commander").identity("G"),
                FixtureCard::new("rampant", "Rampant Growth").identity("G"),
            ])
            .build();
        let recommander_json = serde_json::json!({
            "data": {"recommendations": [
                {"name": "Test Commander", "score": 9.0},
                {"name": "Rampant Growth", "score": 5.0}
            ]}
        })
        .to_string();

        let cache_dir = dir_for_test();
        let result = fetch_all(
            &db,
            &green_deck(&[]),
            [],
            clients(
                &FailingClient,
                &StubRecommanderClient(recommander_json),
                &cache_dir,
            ),
        );

        let names: Vec<_> = result
            .recommander_recommendations
            .iter()
            .map(|r| r.card.name.as_str())
            .collect();
        assert_eq!(names, vec!["Rampant Growth"]);
    }

    #[test]
    fn both_sources_failing_produce_two_source_errors() {
        let (_dir, db) = fixture_db();
        let cache_dir = dir_for_test();
        let result = fetch_all(
            &db,
            &green_deck(&[]),
            [],
            clients(&FailingClient, &FailingClient, &cache_dir),
        );
        assert_eq!(result.source_errors.len(), 2);
        assert!(result.edhrec_recommendations.is_empty());
        assert!(result.recommander_recommendations.is_empty());
    }

    #[test]
    fn malformed_json_produces_a_source_error() {
        let (_dir, db) = fixture_db();
        let cache_dir = dir_for_test();
        let result = fetch_all(
            &db,
            &green_deck(&[]),
            [],
            clients(
                &StubEdhrecClient("not json".to_string()),
                &FailingClient,
                &cache_dir,
            ),
        );
        assert_eq!(result.source_errors.len(), 2);
        assert_eq!(result.source_errors[0].source, "edhrec");
    }

    fn dir_for_test() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}

/// Clients bouchonnés des Sources externes, partagés par les tests.
#[cfg(test)]
pub mod test_clients {
    use anyhow::Result;

    use super::edhrec::EdhrecClient;
    use super::recommander::RecommanderClient;

    /// Sert toujours le même JSON.
    pub struct StubEdhrecClient(pub String);
    impl EdhrecClient for StubEdhrecClient {
        fn fetch(&self, _page: &str) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    /// Sert toujours le même JSON.
    pub struct StubRecommanderClient(pub String);
    impl RecommanderClient for StubRecommanderClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    /// Échoue comme une Source indisponible.
    pub struct FailingClient;
    impl EdhrecClient for FailingClient {
        fn fetch(&self, _page: &str) -> Result<String> {
            anyhow::bail!("HTTP 429")
        }
    }
    impl RecommanderClient for FailingClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            anyhow::bail!("HTTP 429")
        }
    }

    /// Hors ligne : tout appel est une erreur de test.
    pub struct OfflineClient;
    impl EdhrecClient for OfflineClient {
        fn fetch(&self, _page: &str) -> Result<String> {
            panic!("hors ligne : EDHREC ne doit jamais être appelé")
        }
    }
    impl RecommanderClient for OfflineClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            panic!("hors ligne : Recommander ne doit jamais être appelé")
        }
    }
}
