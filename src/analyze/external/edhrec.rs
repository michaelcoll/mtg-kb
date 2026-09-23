//! Client EDHREC : endpoint JSON non officiel
//! `json.edhrec.com/pages/commanders/<slug>.json`, recommandant par
//! Commandant seul.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{cache, resolve_and_filter};
use crate::analyze::ranking::sort_desc_by_score_then_name;
use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::model::EdhrecRecommendation;

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
    db: &CardsDb,
    deck: &DeckContext,
) -> Result<(Vec<EdhrecRecommendation>, Vec<String>)> {
    let commander_slug = slug(deck.commander_name());
    let raw_json = cache::cached_fetch(client, cache_dir, &commander_slug, ttl)?;
    let items = parse(&raw_json)?;

    let (resolved, unresolved_names) = resolve_and_filter(items, db, deck)?;

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

    fn fixture_db() -> (tempfile::TempDir, CardsDb) {
        use crate::db::fixture::{CardsFixture, FixtureCard};
        CardsFixture::new()
            .cards([
                FixtureCard::new("rampant", "Rampant Growth").identity("G"),
                FixtureCard::new("solring", "Sol Ring"),
            ])
            .build()
    }

    /// Enregistre le slug demandé : il vient du nom du Commandant.
    struct RecordingClient {
        json: String,
        slugs: std::cell::RefCell<Vec<String>>,
    }
    impl EdhrecClient for RecordingClient {
        fn fetch(&self, slug: &str) -> Result<String> {
            self.slugs.borrow_mut().push(slug.to_string());
            Ok(self.json.clone())
        }
    }

    #[test]
    fn fetch_and_filter_queries_the_commander_slug() {
        let (_dir, db) = fixture_db();
        let client = RecordingClient {
            json: sample_json(),
            slugs: Default::default(),
        };
        let cache_dir = tempfile::tempdir().unwrap();
        let commander = crate::model::Card::named("Krenko, Mob Boss", &["R"]);
        fetch_and_filter(
            &client,
            cache_dir.path(),
            Duration::from_secs(7 * 86400),
            &db,
            &DeckContext::new(&commander, []),
        )
        .unwrap();
        assert_eq!(*client.slugs.borrow(), vec!["krenko-mob-boss".to_string()]);
    }

    #[test]
    fn fetch_and_filter_keeps_in_identity_cards_and_lists_unresolved_names() {
        let (_dir, db) = fixture_db();
        let client = StubClient(sample_json());
        let cache_dir = tempfile::tempdir().unwrap();
        let commander = crate::model::Card::named("Test Commander", &["G"]);
        let (recommendations, unresolved) = fetch_and_filter(
            &client,
            cache_dir.path(),
            Duration::from_secs(7 * 86400),
            &db,
            &DeckContext::new(&commander, []),
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
