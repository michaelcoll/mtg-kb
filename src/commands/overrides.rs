use std::path::Path;

use anyhow::{Result, bail};

use crate::cli::{Legality, OverrideAction};
use crate::data_dir::{cards_db_path, overrides_db_path};
use crate::db::cards::CardsDb;
use crate::db::overrides::{CorrectionEntry, CorrectionField, CorrectionValue, OverridesDb};
use crate::output::{Format, print_json};

pub fn run(action: OverrideAction) -> Result<()> {
    match action {
        OverrideAction::Role(args) => {
            let roles = parse_labels(args.values.as_deref())?;
            run_set(&args.card, &CorrectionValue::Roles(roles), &args.reason)
        }
        OverrideAction::Theme(args) => {
            let themes = parse_labels(args.values.as_deref())?;
            run_set(&args.card, &CorrectionValue::Themes(themes), &args.reason)
        }
        OverrideAction::Legality {
            card,
            value,
            reason,
        } => run_set(
            &card,
            &CorrectionValue::LegalInCommander(value == Legality::Legal),
            &reason,
        ),
        OverrideAction::List { format } => run_list(format),
        OverrideAction::Remove { field, card } => run_remove(field, &card),
    }
}

fn run_set(card: &str, value: &CorrectionValue, reason: &str) -> Result<()> {
    let cards = CardsDb::open(&cards_db_path())?;
    let name = correct(&cards, &overrides_db_path(), card, value, reason)?;
    println!("Correction posée pour « {name} »");
    Ok(())
}

fn run_list(format: Format) -> Result<()> {
    let entries = list(&overrides_db_path())?;
    match format {
        Format::Json => print_json(&entries),
        Format::Table => {
            for e in &entries {
                println!(
                    "{:<30} {:<9} {:<30} {}  {}",
                    e.card, e.field, e.value, e.date, e.reason
                );
            }
            Ok(())
        }
    }
}

fn run_remove(field: CorrectionField, card: &str) -> Result<()> {
    let cards = CardsDb::open(&cards_db_path())?;
    let name = remove(&cards, &overrides_db_path(), field, card)?;
    println!("Correction retirée pour « {name} »");
    Ok(())
}

/// `None` (`--none`) donne une liste vide.
fn parse_labels(values: Option<&str>) -> Result<Vec<String>> {
    let Some(values) = values else {
        return Ok(Vec::new());
    };
    let mut labels: Vec<String> = Vec::new();
    for label in values.split(',').map(normalize_label) {
        if !label.is_empty() && !labels.contains(&label) {
            labels.push(label);
        }
    }
    if labels.is_empty() {
        bail!("liste de valeurs vide : utilisez --none pour fixer une liste vide");
    }
    Ok(labels)
}

/// snake_case minuscule ; le sous-type d'un Thème `tribal:` garde la casse
/// des sous-types MTGJSON pour correspondre aux Thèmes détectés.
fn normalize_label(raw: &str) -> String {
    let raw = raw.trim();
    if let Some((prefix, subtype)) = raw.split_once(':')
        && prefix.trim().eq_ignore_ascii_case("tribal")
    {
        return format!("tribal:{}", subtype.trim());
    }
    raw.split(|c: char| c.is_whitespace() || c == '-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase()
}

/// Nom complet de la Carte, résolue comme une ligne de Decklist.
fn resolve(cards: &CardsDb, card: &str) -> Result<String> {
    match cards.card(card)? {
        Some(card) => Ok(card.name),
        None => bail!("Carte introuvable : « {card} »"),
    }
}

/// Pose la Correction et renvoie le nom complet de la Carte corrigée.
fn correct(
    cards: &CardsDb,
    overrides_path: &Path,
    card: &str,
    value: &CorrectionValue,
    reason: &str,
) -> Result<String> {
    let reason = reason.trim();
    if reason.is_empty() {
        bail!("un motif est obligatoire (--reason)");
    }
    let name = resolve(cards, card)?;
    OverridesDb::create(overrides_path)?.set(&name, value, reason)?;
    Ok(name)
}

fn list(overrides_path: &Path) -> Result<Vec<CorrectionEntry>> {
    match OverridesDb::open(overrides_path)? {
        Some(db) => db.list(),
        None => Ok(Vec::new()),
    }
}

fn remove(
    cards: &CardsDb,
    overrides_path: &Path,
    field: CorrectionField,
    card: &str,
) -> Result<String> {
    let name = resolve(cards, card)?;
    let removed =
        overrides_path.exists() && OverridesDb::create(overrides_path)?.remove(&name, field)?;
    if !removed {
        bail!("aucune Correction {} pour « {name} »", field.as_str());
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::fixture::{CardsFixture, FixtureCard};
    use crate::db::overrides::load_corrections;

    fn fixture() -> (tempfile::TempDir, CardsDb, std::path::PathBuf) {
        let (dir, db) = CardsFixture::new()
            .cards([
                FixtureCard::new("bite", "Bite Down").types("Instant"),
                FixtureCard::new("studious-a", "Studious First-Year // Rampant Growth")
                    .face("adventure", "a", "Studious First-Year", 1.0)
                    .types("Creature"),
                FixtureCard::new("studious-b", "Studious First-Year // Rampant Growth")
                    .face("adventure", "b", "Rampant Growth", 2.0)
                    .types("Sorcery"),
            ])
            .build();
        let path = dir.path().join("overrides.sqlite");
        (dir, db, path)
    }

    fn roles(values: &[&str]) -> CorrectionValue {
        CorrectionValue::Roles(values.iter().map(|v| v.to_string()).collect())
    }

    #[test]
    fn labels_are_normalized_to_lowercase_snake_case() {
        assert_eq!(normalize_label("grave hate"), "grave_hate");
        assert_eq!(normalize_label("  Removal-Cible "), "removal_cible");
        assert_eq!(normalize_label("+1/+1"), "+1/+1");
    }

    #[test]
    fn a_tribal_theme_keeps_its_subtype_as_detected() {
        assert_eq!(normalize_label("Tribal: Spider"), "tribal:Spider");
    }

    #[test]
    fn values_are_split_on_commas_deduplicated_and_normalized() {
        assert_eq!(
            parse_labels(Some("ramp, grave hate,Ramp")).unwrap(),
            vec!["ramp", "grave_hate"]
        );
        assert!(parse_labels(None).unwrap().is_empty());
    }

    #[test]
    fn an_empty_value_list_is_refused_in_favor_of_none() {
        let err = parse_labels(Some(" , ")).unwrap_err();
        assert!(err.to_string().contains("--none"));
    }

    #[test]
    fn a_correction_is_stored_under_the_full_card_name() {
        let (_dir, db, path) = fixture();
        let name = correct(
            &db,
            &path,
            "Studious First-Year",
            &roles(&["ramp"]),
            "prepared",
        )
        .unwrap();
        assert_eq!(name, "Studious First-Year // Rampant Growth");
        let corrections = load_corrections(&path).unwrap();
        assert_eq!(
            corrections["Studious First-Year // Rampant Growth"].roles,
            Some(vec!["ramp".to_string()])
        );
    }

    #[test]
    fn an_unknown_card_is_refused_without_creating_the_base() {
        let (_dir, db, path) = fixture();
        let err = correct(&db, &path, "Rampant Growth", &roles(&["ramp"]), "x").unwrap_err();
        assert!(err.to_string().contains("Rampant Growth"));
        assert!(!path.exists());
    }

    #[test]
    fn a_blank_reason_is_refused() {
        let (_dir, db, path) = fixture();
        let err = correct(&db, &path, "Bite Down", &roles(&["removal_cible"]), "  ").unwrap_err();
        assert!(err.to_string().contains("motif"));
    }

    #[test]
    fn list_is_empty_without_base() {
        let (_dir, _db, path) = fixture();
        assert!(list(&path).unwrap().is_empty());
    }

    #[test]
    fn remove_reverts_to_detection_and_errors_when_nothing_to_remove() {
        let (_dir, db, path) = fixture();
        correct(&db, &path, "Bite Down", &roles(&["removal_cible"]), "bite").unwrap();
        remove(&db, &path, CorrectionField::Role, "Bite Down").unwrap();
        assert!(list(&path).unwrap().is_empty());
        let err = remove(&db, &path, CorrectionField::Role, "Bite Down").unwrap_err();
        assert!(err.to_string().contains("Bite Down"));
    }
}
