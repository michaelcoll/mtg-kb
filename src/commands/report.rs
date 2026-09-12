use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::model::EnrichedAnalysis;
use crate::report;

const REPORTS_DIR: &str = "reports";

pub fn run(json_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(json_path)
        .with_context(|| format!("lecture du fichier {json_path}"))?;
    let enriched: EnrichedAnalysis = serde_json::from_str(&content)
        .with_context(|| format!("« {json_path} » n'est pas un JSON d'analyse enrichi valide"))?;

    let html = report::render(&enriched);

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
