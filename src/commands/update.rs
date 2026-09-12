use std::io;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;

use crate::data_dir::{cards_db_path, data_dir};

const CARDS_DB_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.sqlite";

pub fn run_cards() -> Result<()> {
    let final_path = cards_db_path();
    std::fs::create_dir_all(data_dir())?;
    let tmp_path = final_path.with_extension("tmp");

    println!("Téléchargement de la Base cartes depuis {CARDS_DB_URL}...");
    download_to_file(CARDS_DB_URL, &tmp_path)?;

    match swap_if_newer(&tmp_path, &final_path)? {
        SwapOutcome::AlreadyUpToDate(version) => {
            println!("Base cartes déjà à jour (version {version}).")
        }
        SwapOutcome::Updated(version) => println!("Base cartes mise à jour : version {version}."),
    }
    Ok(())
}

#[derive(Debug)]
enum SwapOutcome {
    AlreadyUpToDate(String),
    Updated(String),
}

/// Compare la version du fichier téléchargé (`tmp_path`) à celle en place
/// (`final_path`, si elle existe), puis remplace atomiquement ou nettoie le
/// `.tmp` selon le cas.
fn swap_if_newer(tmp_path: &Path, final_path: &Path) -> Result<SwapOutcome> {
    let new_version = read_meta_version(tmp_path)
        .with_context(|| "le fichier téléchargé n'est pas une base cartes MTGJSON valide")?;

    if final_path.exists()
        && let Ok(current_version) = read_meta_version(final_path)
        && current_version == new_version
    {
        std::fs::remove_file(tmp_path)?;
        return Ok(SwapOutcome::AlreadyUpToDate(new_version));
    }

    std::fs::rename(tmp_path, final_path)
        .with_context(|| format!("remplacement atomique de {}", final_path.display()))?;
    Ok(SwapOutcome::Updated(new_version))
}

pub fn run_all() -> Result<()> {
    run_cards()?;
    super::update_rules::run(None)?;
    Ok(())
}

fn download_to_file(url: &str, dest: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(url)
        .with_context(|| format!("téléchargement de {url}"))?
        .error_for_status()
        .with_context(|| format!("réponse HTTP invalide pour {url}"))?;
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("création de {}", dest.display()))?;
    io::copy(&mut response, &mut file).with_context(|| "écriture du fichier téléchargé")?;
    Ok(())
}

fn read_meta_version(path: &Path) -> Result<String> {
    if !path.exists() {
        bail!("fichier introuvable : {}", path.display());
    }
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture de {}", path.display()))?;
    conn.query_row("SELECT version FROM meta LIMIT 1", [], |row| row.get(0))
        .with_context(|| "table meta absente ou invalide")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_cards_db(dir: &Path, name: &str, version: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE meta (date TEXT, version TEXT);")
            .unwrap();
        conn.execute(
            "INSERT INTO meta (date, version) VALUES ('2026-01-01', ?1)",
            [version],
        )
        .unwrap();
        path
    }

    #[test]
    fn swap_replaces_when_no_existing_db() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = fake_cards_db(dir.path(), "AllPrintings.tmp", "5.3.0");
        let final_path = dir.path().join("AllPrintings.sqlite");

        let outcome = swap_if_newer(&tmp, &final_path).unwrap();
        assert!(matches!(outcome, SwapOutcome::Updated(v) if v == "5.3.0"));
        assert!(final_path.exists());
        assert!(!tmp.exists());
    }

    #[test]
    fn swap_skips_when_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = fake_cards_db(dir.path(), "AllPrintings.sqlite", "5.3.0");
        let tmp = fake_cards_db(dir.path(), "AllPrintings.tmp", "5.3.0");

        let outcome = swap_if_newer(&tmp, &final_path).unwrap();
        assert!(matches!(outcome, SwapOutcome::AlreadyUpToDate(v) if v == "5.3.0"));
        assert!(!tmp.exists(), "le .tmp doit être nettoyé si non utilisé");
    }

    #[test]
    fn swap_replaces_when_different_version() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = fake_cards_db(dir.path(), "AllPrintings.sqlite", "5.2.0");
        let tmp = fake_cards_db(dir.path(), "AllPrintings.tmp", "5.3.0");

        let outcome = swap_if_newer(&tmp, &final_path).unwrap();
        assert!(matches!(outcome, SwapOutcome::Updated(v) if v == "5.3.0"));
        assert_eq!(read_meta_version(&final_path).unwrap(), "5.3.0");
    }

    #[test]
    fn invalid_downloaded_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("AllPrintings.tmp");
        std::fs::write(&tmp, b"not a sqlite file").unwrap();
        let final_path = dir.path().join("AllPrintings.sqlite");

        let err = swap_if_newer(&tmp, &final_path).unwrap_err();
        assert!(err.to_string().contains("valide"));
    }
}
