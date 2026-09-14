use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::model::EnrichedAnalysis;
use crate::report;
use crate::report::SuggestionPrintings;

const REPORTS_DIR: &str = "reports";

pub fn run(json_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(json_path)
        .with_context(|| format!("lecture du fichier {json_path}"))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("« {json_path} » n'est pas un JSON valide"))?;
    if value.get("verdict").is_some_and(|v| v.is_string()) {
        bail!(
            "« {json_path} » utilise l'ancien format de Verdict (texte libre), qui n'est plus \
             accepté. Le Verdict doit être un objet {{summary, strengths, weaknesses, \
             priorities}} — voir le skill mtg-deck-analyze."
        );
    }
    let enriched: EnrichedAnalysis = serde_json::from_value(value)
        .with_context(|| format!("« {json_path} » n'est pas un JSON d'analyse enrichi valide"))?;

    let cards_db = CardsDb::open(&cards_db_path())?;
    let violations = report::validate::validate_suggestions(&enriched, &cards_db);
    if !violations.is_empty() {
        let messages = violations.join("\n");
        bail!("Suggestions invalides, aucun rapport écrit :\n{messages}");
    }

    let commander_printing = cards_db.reference_printing(&enriched.analysis.commander.name)?;
    let origins = report::origins::compute_origins(&enriched);
    let suggestion_printings = enriched
        .suggestions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let printing = cards_db.reference_printing(&s.card_name)?;
            let card_to_remove_printing = match &s.card_to_remove {
                Some(name) => cards_db.reference_printing(name)?,
                None => None,
            };
            Ok(SuggestionPrintings {
                printing,
                card_to_remove_printing,
                origins: origins.get(i).cloned().unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let html = report::render(
        &enriched,
        commander_printing.as_ref(),
        &suggestion_printings,
    );

    let dir = PathBuf::from(REPORTS_DIR);
    std::fs::create_dir_all(&dir)?;
    let slug = report::slugify(&enriched.analysis.commander.name);
    let date = chrono::Local::now().format("%Y-%m-%d");
    let output_path = dir.join(format!("{slug}-{date}.html"));

    std::fs::write(&output_path, html)
        .with_context(|| format!("écriture de {}", output_path.display()))?;
    println!("Rapport généré : {}", output_path.display());
    Ok(())
}
