pub mod parser;

use std::io::Read as _;

use anyhow::{Context, Result};

/// Texte d'une Decklist : `source` est un chemin de fichier, ou "-" pour
/// lire depuis l'entrée standard.
pub fn read_source(source: &str) -> Result<String> {
    if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de la Decklist depuis l'entrée standard")?;
        Ok(buf)
    } else {
        std::fs::read_to_string(source).with_context(|| format!("lecture du fichier {source}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_decklist_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.txt");
        std::fs::write(&path, "Commander\n1 Doom Blade\n").unwrap();
        assert_eq!(
            read_source(path.to_str().unwrap()).unwrap(),
            "Commander\n1 Doom Blade\n"
        );
    }

    #[test]
    fn a_missing_file_names_the_path_in_the_error() {
        let err = read_source("/nonexistent/deck.txt").unwrap_err();
        assert!(format!("{err:#}").contains("/nonexistent/deck.txt"));
    }
}
