//! Client EDHREC : endpoint JSON non officiel
//! `json.edhrec.com/pages/commanders/<slug>.json` pour la page Commandant, et
//! `<slug>/<thème>.json` pour la page d'un Thème majeur de ce Commandant.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{cache, resolve_and_filter};
use crate::db::cards::CardsDb;
use crate::deck_context::DeckContext;
use crate::model::EdhrecRecommendation;

pub trait EdhrecClient {
    /// `page` : chemin sous `pages/commanders/`, sans extension (`Page::path`).
    fn fetch(&self, page: &str) -> Result<String>;
}

pub struct HttpEdhrecClient;

impl EdhrecClient for HttpEdhrecClient {
    fn fetch(&self, page: &str) -> Result<String> {
        let url = format!("https://json.edhrec.com/pages/commanders/{page}.json");
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

/// Dédupliqué par nom en gardant la meilleure synergie. Trié par synergie
/// décroissante, puis taux d'inclusion décroissant, puis nom.
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
    items.sort_by(|(a_name, a), (b_name, b)| {
        b.synergy
            .total_cmp(&a.synergy)
            .then_with(|| b.inclusion_rate.total_cmp(&a.inclusion_rate))
            .then_with(|| a_name.cmp(b_name))
    });
    Ok(items)
}

/// Suffixe de page EDHREC des Thèmes non tribaux.
const THEME_PAGES: &[(&str, &str)] = &[
    ("tokens", "tokens"),
    ("+1/+1", "plus-1-plus-1-counters"),
    ("aristocrats", "aristocrats"),
    ("artefacts", "artifacts"),
    ("enchantements", "enchantress"),
    ("spellslinger", "spellslinger"),
    ("cimetiere", "reanimator"),
    ("landfall", "landfall"),
    ("lifegain", "lifegain"),
    ("blink", "blink"),
    ("voltron", "voltron"),
];

/// Suffixe de page EDHREC des Thèmes `tribal:<sous-type>`, limités aux
/// tribus qu'EDHREC traite comme thème.
const TRIBAL_PAGES: &[(&str, &str)] = &[
    ("Angel", "angels"),
    ("Assassin", "assassins"),
    ("Beast", "beasts"),
    ("Bird", "birds"),
    ("Cat", "cats"),
    ("Cleric", "clerics"),
    ("Demon", "demons"),
    ("Dinosaur", "dinosaurs"),
    ("Dog", "dogs"),
    ("Dragon", "dragons"),
    ("Dwarf", "dwarves"),
    ("Eldrazi", "eldrazi"),
    ("Elemental", "elementals"),
    ("Elf", "elves"),
    ("Faerie", "faeries"),
    ("Giant", "giants"),
    ("Goblin", "goblins"),
    ("Horror", "horrors"),
    ("Human", "humans"),
    ("Hydra", "hydras"),
    ("Insect", "insects"),
    ("Knight", "knights"),
    ("Merfolk", "merfolk"),
    ("Ninja", "ninjas"),
    ("Pirate", "pirates"),
    ("Rat", "rats"),
    ("Rogue", "rogues"),
    ("Saproling", "saprolings"),
    ("Shaman", "shamans"),
    ("Sliver", "slivers"),
    ("Snake", "snakes"),
    ("Soldier", "soldiers"),
    ("Sphinx", "sphinxes"),
    ("Spider", "spiders"),
    ("Spirit", "spirits"),
    ("Squirrel", "squirrels"),
    ("Treefolk", "treefolk"),
    ("Vampire", "vampires"),
    ("Warrior", "warriors"),
    ("Werewolf", "werewolves"),
    ("Wizard", "wizards"),
    ("Wolf", "wolves"),
    ("Zombie", "zombies"),
];

/// Suffixe de la page EDHREC d'un Thème, relatif à la page Commandant ;
/// `None` pour un Thème sans équivalent EDHREC, qui ne déclenche aucun appel.
pub fn theme_page(theme: &str) -> Option<&'static str> {
    let (table, key) = match theme.strip_prefix("tribal:") {
        Some(subtype) => (TRIBAL_PAGES, subtype),
        None => (THEME_PAGES, theme),
    };
    table
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, page)| *page)
}

/// Une page EDHREC à interroger : la page Commandant (`theme` absent) ou la
/// page d'un Thème majeur pour ce Commandant.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Chemin sous `pages/commanders/`, sans extension ; sert aussi de clé de
    /// cache (une par Commandant et Thème).
    pub path: String,
    pub theme: Option<String>,
}

/// Page Commandant, puis une page par Thème majeur de la table, dans l'ordre
/// alphabétique des Thèmes.
pub fn pages<'a>(
    deck: &DeckContext,
    major_themes: impl IntoIterator<Item = &'a String>,
) -> Vec<Page> {
    let commander_slug = slug(deck.commander_name());
    let mut themes: Vec<&String> = major_themes.into_iter().collect();
    themes.sort();
    themes.dedup();

    let theme_pages = themes.into_iter().filter_map(|theme| {
        theme_page(theme).map(|suffix| Page {
            path: format!("{commander_slug}/{suffix}"),
            theme: Some(theme.clone()),
        })
    });
    std::iter::once(Page {
        path: commander_slug.clone(),
        theme: None,
    })
    .chain(theme_pages)
    .collect()
}

pub fn fetch_and_filter(
    client: &dyn EdhrecClient,
    cache_dir: &Path,
    ttl: Duration,
    db: &CardsDb,
    deck: &DeckContext,
    page: &Page,
) -> Result<(Vec<EdhrecRecommendation>, Vec<String>)> {
    let raw_json = cache::cached_fetch(client, cache_dir, &page.path, ttl)?;
    let items = parse(&raw_json)?;

    let (resolved, unresolved_names) = resolve_and_filter(items, db, deck)?;

    let recommendations = resolved
        .into_iter()
        .map(|(card, meta)| EdhrecRecommendation {
            card,
            synergy: meta.synergy,
            inclusion_rate: meta.inclusion_rate,
            header: meta.header,
            theme: page.theme.clone(),
            origins: vec![],
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
    fn the_table_covers_every_non_tribal_theme() {
        for (theme, page) in [
            ("tokens", "tokens"),
            ("+1/+1", "plus-1-plus-1-counters"),
            ("aristocrats", "aristocrats"),
            ("artefacts", "artifacts"),
            ("enchantements", "enchantress"),
            ("spellslinger", "spellslinger"),
            ("cimetiere", "reanimator"),
            ("landfall", "landfall"),
            ("lifegain", "lifegain"),
            ("blink", "blink"),
            ("voltron", "voltron"),
        ] {
            assert_eq!(theme_page(theme), Some(page), "{theme}");
        }
    }

    #[test]
    fn tribal_themes_map_to_their_plural_page_when_known() {
        assert_eq!(theme_page("tribal:Elf"), Some("elves"));
        assert_eq!(theme_page("tribal:Zombie"), Some("zombies"));
        assert_eq!(theme_page("tribal:Homunculus"), None);
        assert_eq!(theme_page("inconnu"), None);
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
        let deck = DeckContext::new(&commander, []);
        fetch_and_filter(
            &client,
            cache_dir.path(),
            Duration::from_secs(7 * 86400),
            &db,
            &deck,
            &pages(&deck, [])[0],
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
        let deck = DeckContext::new(&commander, []);
        let (recommendations, unresolved) = fetch_and_filter(
            &client,
            cache_dir.path(),
            Duration::from_secs(7 * 86400),
            &db,
            &deck,
            &pages(&deck, [])[0],
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
