//! Non-régression sur mes Decklists personnelles (`data/decks/*.txt`, hors
//! dépôt) contre la vraie Base cartes, hors ligne, par la même seam que
//! `kb analyze --offline`.
//!
//! Ignoré par défaut ; sauté sans échouer si `data/decks/` ou la Base cartes
//! manque. Lancement : `mise run regression`, soit
//! `cargo test personal_decklists -- --ignored --nocapture`.
//! Le résumé des Candidats imprimé par Decklist sert à comparer avant/après.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::analyze::MAJOR_THEME_MIN_CARDS;
use crate::analyze::external::edhrec::EdhrecClient;
use crate::analyze::external::recommander::RecommanderClient;
use crate::analyze::metrics::{Thresholds, weak_role_names};
use crate::data_dir::{cards_db_path, data_dir};
use crate::model::AnalyzeResult;

const DECKS_SUBDIR: &str = "decks";

struct NoNetworkEdhrecClient;
impl EdhrecClient for NoNetworkEdhrecClient {
    fn fetch(&self, _slug: &str) -> anyhow::Result<String> {
        panic!("la non-régression est hors ligne : EDHREC ne doit pas être appelé")
    }
}

struct NoNetworkRecommanderClient;
impl RecommanderClient for NoNetworkRecommanderClient {
    fn fetch(&self, _body: &serde_json::Value) -> anyhow::Result<String> {
        panic!("la non-régression est hors ligne : Recommander ne doit pas être appelé")
    }
}

fn personal_decklists(decks_dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(decks_dir)
        .unwrap_or_else(|e| panic!("lecture de {} : {e}", decks_dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    paths.sort();
    paths
}

/// Rôles sous-représentés et Thèmes majeurs (au sens du Deck : au moins
/// `MAJOR_THEME_MIN_CARDS` Cartes) qui n'ont aucun Candidat.
fn buckets_without_candidates(result: &AnalyzeResult, thresholds: &Thresholds) -> Vec<String> {
    let mut theme_counts: BTreeMap<&str, u32> = BTreeMap::new();
    for resolved in &result.cards {
        for theme in &resolved.themes {
            *theme_counts.entry(theme).or_insert(0) += resolved.quantity;
        }
    }
    let major_themes = theme_counts
        .into_iter()
        .filter(|(_, count)| *count >= MAJOR_THEME_MIN_CARDS)
        .map(|(theme, _)| theme.to_string());

    let mut missing = Vec::new();
    for role in weak_role_names(&result.role_counts, thresholds) {
        if !result
            .candidates
            .iter()
            .any(|c| c.matched_weak_roles.contains(&role))
        {
            missing.push(format!("Rôle sous-représenté « {role} »"));
        }
    }
    for theme in major_themes {
        if !result
            .candidates
            .iter()
            .any(|c| c.matched_themes.contains(&theme))
        {
            missing.push(format!("Thème majeur « {theme} »"));
        }
    }
    missing
}

/// Candidats par Rôle sous-représenté et par Thème majeur, dans l'ordre.
fn print_summary(deck: &str, result: &AnalyzeResult) {
    let mut buckets: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for candidate in &result.candidates {
        let labels = candidate
            .matched_weak_roles
            .iter()
            .map(|r| format!("rôle {r}"))
            .chain(
                candidate
                    .matched_themes
                    .iter()
                    .map(|t| format!("thème {t}")),
            );
        for label in labels {
            buckets.entry(label).or_default().push(&candidate.card.name);
        }
    }
    println!("== {deck} ({} Candidats)", result.candidates.len());
    let unresolved: BTreeSet<&str> = result.unresolved.iter().map(|u| u.name.as_str()).collect();
    if !unresolved.is_empty() {
        println!("   non résolues : {unresolved:?}");
    }
    for (label, names) in buckets {
        println!("   {label} : {}", names.join(" | "));
    }
}

#[test]
#[ignore = "Decklists personnelles hors dépôt : mise run regression"]
fn personal_decklists_produce_candidates_for_each_weak_role_and_major_theme() {
    let decks_dir = data_dir().join(DECKS_SUBDIR);
    if !decks_dir.is_dir() || !cards_db_path().is_file() {
        eprintln!(
            "sauté : {} ou {} absent",
            decks_dir.display(),
            cards_db_path().display()
        );
        return;
    }

    let db = super::super::open_cards_db().expect("ouverture de la Base cartes");
    let thresholds = Thresholds::default();
    let cache_dir = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();

    for path in personal_decklists(&decks_dir) {
        let deck = path.file_name().unwrap().to_string_lossy().into_owned();
        let analysis = crate::decklist::read_source(&path.to_string_lossy()).and_then(|input| {
            super::analyze_deck(
                &input,
                &db,
                &thresholds,
                true,
                &NoNetworkEdhrecClient,
                &NoNetworkRecommanderClient,
                cache_dir.path(),
            )
        });
        match analysis {
            Err(e) => failures.push(format!("{deck} : l'analyse échoue : {e:#}")),
            Ok(result) => {
                print_summary(&deck, &result);
                for missing in buckets_without_candidates(&result, &thresholds) {
                    failures.push(format!("{deck} : aucun Candidat pour le {missing}"));
                }
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
