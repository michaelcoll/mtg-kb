//! Client Recommander : recommande à partir de la Decklist complète (sans
//! terrains de base).

use std::collections::HashSet;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::resolve_and_filter;
use crate::analyze::ranking::sort_desc_by_score_then_name;
use crate::db::cards::CardsDb;
use crate::model::{AnalyzeResult, RecommanderRecommendation};

const RECOMMANDER_URL: &str =
    "https://api.recommander.cards/public-release/api/decks/recommend/top";

pub trait RecommanderClient {
    fn fetch(&self, body: &serde_json::Value) -> Result<String>;
}

pub struct HttpRecommanderClient;

impl RecommanderClient for HttpRecommanderClient {
    fn fetch(&self, body: &serde_json::Value) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(RECOMMANDER_URL)
            .json(body)
            .send()
            .context("appel Recommander")?
            .error_for_status()
            .context("réponse HTTP invalide de Recommander")?;
        response.text().context("lecture de la réponse Recommander")
    }
}

pub(super) fn request_body(analysis: &AnalyzeResult) -> serde_json::Value {
    let deck: Vec<String> = analysis
        .cards
        .iter()
        .filter(|c| !c.card.is_basic_land())
        .map(|c| c.card.name.clone())
        .collect();
    serde_json::json!({
        "card_format": "name",
        "commander": analysis.commander.name,
        "deck": deck,
    })
}

#[derive(Debug, Deserialize)]
struct RecommanderResponse {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    #[serde(default)]
    recommendations: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    name: String,
    score: f64,
    #[allow(dead_code)]
    #[serde(default)]
    oracle_id: Option<String>,
}

fn parse(raw_json: &str) -> Result<Vec<(String, f64)>> {
    let parsed: RecommanderResponse =
        serde_json::from_str(raw_json).context("structure JSON Recommander inattendue")?;
    let mut items: Vec<(String, f64)> = parsed
        .data
        .recommendations
        .into_iter()
        .map(|i| (i.name, i.score))
        .collect();
    sort_desc_by_score_then_name(&mut items, |i| i.1, |i| &i.0);
    Ok(items)
}

/// La liste est vide si la Decklist est trop courte (comportement de
/// Recommander, pas une erreur).
pub fn fetch_and_filter(
    client: &dyn RecommanderClient,
    analysis: &AnalyzeResult,
    db: &CardsDb,
    deck_names: &HashSet<String>,
) -> Result<(Vec<RecommanderRecommendation>, Vec<String>)> {
    let body = request_body(analysis);
    let raw_json = client.fetch(&body)?;
    let items = parse(&raw_json)?;

    let (resolved, unresolved_names) =
        resolve_and_filter(items, db, &analysis.commander.color_identity, deck_names)?;

    let recommendations = resolved
        .into_iter()
        .map(|(card, score)| RecommanderRecommendation { card, score })
        .collect();
    Ok((recommendations, unresolved_names))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::model::*;

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        CardsFixture::new()
            .cards([
                FixtureCard::new("rampant", "Rampant Growth").identity("G"),
                FixtureCard::new("bolt", "Lightning Bolt").identity("R"),
            ])
            .build()
    }

    fn sample_analysis() -> AnalyzeResult {
        let forest = Card::from_face(Face {
            name: "Forest".to_string(),
            types: vec!["Land".to_string()],
            supertypes: vec!["Basic".to_string()],
            ..Face::default()
        });
        AnalyzeResult {
            commander: Card::named("Test Commander", &["G"]),
            cards: vec![
                ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: forest,
                },
                ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: Card::named("Some Nonland Card", &["G"]),
                },
            ],
            unresolved: vec![],
            card_count: 100,
            construction_errors: vec![],
            mana_curve: ManaCurve {
                buckets: vec![],
                average_mana_value: 0.0,
            },
            mana_base: ManaBase {
                land_count: 1,
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
    fn request_body_omits_basic_lands_and_partner() {
        let body = request_body(&sample_analysis());
        assert_eq!(body["commander"], "Test Commander");
        assert_eq!(body["card_format"], "name");
        assert_eq!(body["deck"], serde_json::json!(["Some Nonland Card"]));
        assert!(body.get("partner").is_none());
    }

    struct StubClient(String);
    impl RecommanderClient for StubClient {
        fn fetch(&self, _body: &serde_json::Value) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn parses_sorts_by_score_and_filters_out_of_identity() {
        let (_dir, db) = fixture_db();
        let raw = serde_json::json!({
            "data": {
                "recommendations": [
                    {"oracle_id": "a", "name": "Rampant Growth", "score": 1.5},
                    {"oracle_id": "b", "name": "Lightning Bolt", "score": 9.0},
                    {"oracle_id": "c", "name": "Unresolvable Card", "score": 0.5}
                ]
            }
        })
        .to_string();
        let client = StubClient(raw);
        let (recommendations, unresolved) =
            fetch_and_filter(&client, &sample_analysis(), &db, &HashSet::new()).unwrap();

        let names: Vec<&str> = recommendations
            .iter()
            .map(|r| r.card.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["Rampant Growth"],
            "Lightning Bolt is red, outside the G commander's identity"
        );
        assert_eq!(unresolved, vec!["Unresolvable Card".to_string()]);
    }

    #[test]
    fn an_empty_recommendations_list_is_not_an_error() {
        let (_dir, db) = fixture_db();
        let raw = serde_json::json!({"data": {"recommendations": []}}).to_string();
        let client = StubClient(raw);
        let (recommendations, unresolved) =
            fetch_and_filter(&client, &sample_analysis(), &db, &HashSet::new()).unwrap();
        assert!(recommendations.is_empty());
        assert!(unresolved.is_empty());
    }

    #[test]
    fn malformed_json_is_an_error() {
        let err = parse("not json").unwrap_err();
        assert!(err.to_string().contains("Recommander"));
    }
}
