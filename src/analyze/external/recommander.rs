//! Client Recommander (voir ADR 0003) : API publique sans clé,
//! `POST https://api.recommander.cards/public-release/api/decks/recommend/top`,
//! recommandant à partir de la Decklist complète (sans terrains de base).
//! Pas de cache : la réponse dépend de la Decklist, jamais identique d'un
//! appel à l'autre.

use std::collections::HashSet;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::resolve_and_filter;
use crate::analyze::is_basic_land;
use crate::db::cards::CardsDb;
use crate::model::{AnalyzeResult, RecommanderRecommendation};

const RECOMMANDER_URL: &str =
    "https://api.recommander.cards/public-release/api/decks/recommend/top";

pub trait RecommanderClient {
    /// Envoie `body` à l'API Recommander et renvoie le corps JSON brut de la
    /// réponse.
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

/// Corps de la requête Recommander : `deck` exclut les terrains de base
/// (voir ADR 0003). Recommander modélise un Commandant `partner`, non
/// supporté par cette Base cartes (`analyze::run` n'accepte qu'un seul
/// Commandant) : `partner` n'est jamais envoyé.
pub(super) fn request_body(analysis: &AnalyzeResult) -> serde_json::Value {
    let deck: Vec<String> = analysis
        .cards
        .iter()
        .filter(|c| !is_basic_land(&c.card))
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

/// Parse `data.recommendations[]` et trie par score décroissant.
fn parse(raw_json: &str) -> Result<Vec<(String, f64)>> {
    let parsed: RecommanderResponse =
        serde_json::from_str(raw_json).context("structure JSON Recommander inattendue")?;
    let mut items: Vec<(String, f64)> = parsed
        .data
        .recommendations
        .into_iter()
        .map(|i| (i.name, i.score))
        .collect();
    items.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    Ok(items)
}

/// Interroge puis filtre les Recommandations externes Recommander pour la
/// Decklist de `analysis`. Attention : la liste est vide si la Decklist est
/// trop courte (comportement de Recommander, pas une erreur).
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
    use crate::model::*;
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

            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            INSERT INTO cardLegalities VALUES ('bolt', 'Legal');
            "#,
        )
        .unwrap();
        (dir, CardsDb::open(&path).unwrap())
    }

    fn sample_analysis() -> AnalyzeResult {
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
            cards: vec![
                ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: Card {
                        name: "Forest".to_string(),
                        mana_cost: None,
                        mana_value: Some(0.0),
                        type_line: None,
                        types: vec!["Land".to_string()],
                        subtypes: vec!["Forest".to_string()],
                        supertypes: vec!["Basic".to_string()],
                        oracle_text: None,
                        color_identity: vec![],
                        colors: vec![],
                        keywords: vec![],
                        power: None,
                        toughness: None,
                        loyalty: None,
                    },
                },
                ResolvedCard {
                    quantity: 1,
                    roles: vec![],
                    themes: vec![],
                    card: Card {
                        name: "Some Nonland Card".to_string(),
                        mana_cost: None,
                        mana_value: Some(2.0),
                        type_line: None,
                        types: vec!["Creature".to_string()],
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
