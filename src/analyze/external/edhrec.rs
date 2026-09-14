//! Client EDHREC : endpoint JSON non officiel
//! `json.edhrec.com/pages/commanders/<slug>.json`, recommandant par
//! Commandant seul.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{cache, resolve_and_filter};
use crate::analyze::ranking::sort_desc_by_score_then_name;
use crate::db::cards::CardsDb;
use crate::model::{AnalyzeResult, EdhrecRecommendation};

pub trait EdhrecClient {
    fn fetch(&self, slug: &str) -> Result<String>;
}

pub struct HttpEdhrecClient;

impl EdhrecClient for HttpEdhrecClient {
    fn fetch(&self, slug: &str) -> Result<String> {
        let url = format!("https://json.edhrec.com/pages/commanders/{slug}.json");
        let response = reqwest::blocking::get(&url)
            .with_context(|| format!("appel EDHREC ({url})"))?
            .error_for_status()
            .with_context(|| format!("réponse HTTP invalide d'EDHREC ({url})"))?;
        response.text().context("lecture de la réponse EDHREC")
    }
}

/// Ex. "Atraxa, Praetors' Voice" -> "atraxa-praetors-voice".
pub fn slug(commander_name: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = true;
    for c in commander_name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_was_dash = false;
        } else if c.is_whitespace() && !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Debug, Deserialize)]
struct EdhrecResponse {
    container: Container,
}

#[derive(Debug, Deserialize)]
struct Container {
    json_dict: JsonDict,
}

#[derive(Debug, Deserialize)]
struct JsonDict {
    #[serde(default)]
    cardlists: Vec<Cardlist>,
}

#[derive(Debug, Deserialize)]
struct Cardlist {
    #[serde(default)]
    header: Option<String>,
    #[serde(default)]
    cardviews: Vec<Cardview>,
}

#[derive(Debug, Deserialize)]
struct Cardview {
    name: String,
    #[serde(default)]
    synergy: Option<f64>,
    #[serde(default)]
    num_decks: Option<f64>,
    #[serde(default)]
    potential_decks: Option<f64>,
}

#[derive(Debug)]
struct RawMeta {
    synergy: f64,
    inclusion_rate: f64,
    header: String,
}

/// Dédupliqué par nom en gardant la meilleure synergie.
fn parse(raw_json: &str) -> Result<Vec<(String, RawMeta)>> {
    let parsed: EdhrecResponse =
        serde_json::from_str(raw_json).context("structure JSON EDHREC inattendue")?;

    let mut merged: HashMap<String, RawMeta> = HashMap::new();
    for cardlist in parsed.container.json_dict.cardlists {
        let header = cardlist.header.unwrap_or_default();
        for view in cardlist.cardviews {
            let synergy = view.synergy.unwrap_or(0.0);
            let inclusion_rate = match (view.num_decks, view.potential_decks) {
                (Some(n), Some(p)) if p > 0.0 => n / p,
                _ => 0.0,
            };
            merged
                .entry(view.name)
                .and_modify(|existing| {
                    if synergy > existing.synergy {
                        existing.synergy = synergy;
                        existing.inclusion_rate = inclusion_rate;
                        existing.header = header.clone();
                    }
                })
                .or_insert(RawMeta {
                    synergy,
                    inclusion_rate,
                    header: header.clone(),
                });
        }
    }

    let mut items: Vec<(String, RawMeta)> = merged.into_iter().collect();
    sort_desc_by_score_then_name(&mut items, |i| i.1.synergy, |i| &i.0);
    Ok(items)
}

pub fn fetch_and_filter(
    client: &dyn EdhrecClient,
    cache_dir: &Path,
    ttl: Duration,
    analysis: &AnalyzeResult,
    db: &CardsDb,
    deck_names: &HashSet<String>,
) -> Result<(Vec<EdhrecRecommendation>, Vec<String>)> {
    let commander_slug = slug(&analysis.commander.name);
    let raw_json = cache::cached_fetch(client, cache_dir, &commander_slug, ttl)?;
    let items = parse(&raw_json)?;

    let (resolved, unresolved_names) =
        resolve_and_filter(items, db, &analysis.commander.color_identity, deck_names)?;

    let recommendations = resolved
        .into_iter()
        .map(|(card, meta)| EdhrecRecommendation {
            card,
            synergy: meta.synergy,
            inclusion_rate: meta.inclusion_rate,
            header: meta.header,
        })
        .collect();
    Ok((recommendations, unresolved_names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_apostrophes_and_commas() {
        assert_eq!(slug("Atraxa, Praetors' Voice"), "atraxa-praetors-voice");
    }

    #[test]
    fn slugifies_simple_names() {
        assert_eq!(slug("Krenko, Mob Boss"), "krenko-mob-boss");
    }

    fn sample_json() -> String {
        serde_json::json!({
            "container": {
                "json_dict": {
                    "cardlists": [
                        {
                            "header": "High Synergy Cards",
                            "cardviews": [
                                {"name": "Rampant Growth", "synergy": 0.12, "num_decks": 500, "potential_decks": 1000},
                                {"name": "Sol Ring", "synergy": 0.02, "num_decks": 900, "potential_decks": 1000}
                            ]
                        },
                        {
                            "header": "New Cards",
                            "cardviews": [
                                {"name": "Sol Ring", "synergy": 0.30, "num_decks": 900, "potential_decks": 1000},
                                {"name": "Unresolvable Test Card", "synergy": 0.05, "num_decks": 10, "potential_decks": 100}
                            ]
                        }
                    ]
                }
            }
        })
        .to_string()
    }

    #[test]
    fn parses_and_merges_cardlists_sorted_by_synergy_desc() {
        let items = parse(&sample_json()).unwrap();
        let names: Vec<&str> = items.iter().map(|(n, _)| n.as_str()).collect();
        // Sol Ring apparaît deux fois (0.02 et 0.30) : on garde la meilleure
        // synergie (0.30, header "New Cards") et le tri est décroissant.
        assert_eq!(
            names,
            vec!["Sol Ring", "Rampant Growth", "Unresolvable Test Card"]
        );
        let sol_ring = items.iter().find(|(n, _)| n == "Sol Ring").unwrap();
        assert_eq!(sol_ring.1.synergy, 0.30);
        assert_eq!(sol_ring.1.header, "New Cards");
    }

    #[test]
    fn computes_inclusion_rate_from_num_and_potential_decks() {
        let items = parse(&sample_json()).unwrap();
        let rampant = items.iter().find(|(n, _)| n == "Rampant Growth").unwrap();
        assert_eq!(rampant.1.inclusion_rate, 0.5);
    }

    #[test]
    fn malformed_json_is_an_error() {
        let err = parse("not json").unwrap_err();
        assert!(err.to_string().contains("EDHREC"));
    }

    #[test]
    fn unexpected_structure_is_an_error() {
        let err = parse(r#"{"unexpected": true}"#).unwrap_err();
        assert!(err.to_string().contains("EDHREC"));
    }

    struct StubClient(String);
    impl EdhrecClient for StubClient {
        fn fetch(&self, _slug: &str) -> Result<String> {
            Ok(self.0.clone())
        }
    }

    fn fixture_db_and_analysis() -> (tempfile::TempDir, CardsDb, AnalyzeResult) {
        use rusqlite::Connection;
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
            INSERT INTO cards VALUES ('solring', 'Sol Ring', '{1}', 1.0, 'Artifact', 'Artifact',
                NULL, NULL, 'text', NULL, NULL, NULL, NULL, NULL, NULL);

            INSERT INTO cardLegalities VALUES ('rampant', 'Legal');
            INSERT INTO cardLegalities VALUES ('solring', 'Legal');
            "#,
        )
        .unwrap();
        let db = CardsDb::open(&path).unwrap();

        use crate::model::*;
        let analysis = AnalyzeResult {
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
        };
        (dir, db, analysis)
    }

    #[test]
    fn fetch_and_filter_keeps_in_identity_cards_and_lists_unresolved_names() {
        let (_dir, db, analysis) = fixture_db_and_analysis();
        let client = StubClient(sample_json());
        let cache_dir = tempfile::tempdir().unwrap();
        let (recommendations, unresolved) = fetch_and_filter(
            &client,
            cache_dir.path(),
            Duration::from_secs(7 * 86400),
            &analysis,
            &db,
            &HashSet::new(),
        )
        .unwrap();

        let names: Vec<&str> = recommendations
            .iter()
            .map(|r| r.card.name.as_str())
            .collect();
        assert_eq!(names, vec!["Sol Ring", "Rampant Growth"]);
        assert_eq!(unresolved, vec!["Unresolvable Test Card".to_string()]);
    }
}
