use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::data_dir::rules_db_path;
use crate::db::rules::RulesDb;
use crate::rules::parser::{parse_effective_date, parse_glossary, parse_rules, split_document};

const RULES_INDEX_URL: &str = "https://magic.wizards.com/en/rules";

pub fn run(source_url: Option<String>) -> Result<()> {
    let url = match source_url {
        Some(url) => url,
        None => discover_latest_rules_url()?,
    };
    println!("Téléchargement des règles depuis {url}...");
    let full_text = reqwest::blocking::get(&url)
        .with_context(|| format!("téléchargement de {url}"))?
        .error_for_status()
        .with_context(|| format!("réponse HTTP invalide pour {url}"))?
        .text()
        .context("lecture du corps de la réponse")?;

    // Le document Wizards est en CRLF avec BOM UTF-8.
    let full_text = full_text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n");

    let version = version_from_url(&url).unwrap_or_else(|| "inconnue".to_string());
    let effective_date = parse_effective_date(&full_text)
        .ok_or_else(|| anyhow::anyhow!("date d'entrée en vigueur introuvable dans le document"))?;

    let (body, glossary_body) = split_document(&full_text);
    let (sections, rules) = parse_rules(body);
    let glossary = parse_glossary(glossary_body);
    if rules.is_empty() {
        bail!("aucune règle numérotée détectée : le format du document a peut-être changé");
    }

    let final_path = rules_db_path();
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if final_path.exists() {
        let existing = RulesDb::open(&final_path)?;
        if let Some((existing_version, _)) = existing.version()?
            && existing_version == version
        {
            println!("Base règles déjà à jour (version {version}).");
            return Ok(());
        }
    }

    let tmp_path = final_path.with_extension("tmp");
    RulesDb::build(
        &tmp_path,
        &version,
        &effective_date,
        &sections,
        &rules,
        &glossary,
    )?;
    RulesDb::open(&tmp_path)
        .with_context(|| "la base règles nouvellement construite est invalide")?;
    atomic_swap(&tmp_path, &final_path)?;

    println!(
        "Base règles mise à jour : version {version} (effective au {effective_date}), \
         {} règles, {} entrées de glossaire.",
        rules.len(),
        glossary.len()
    );
    Ok(())
}

fn version_from_url(url: &str) -> Option<String> {
    let file_name = url.rsplit('/').next()?.replace("%20", " ");
    regex::Regex::new(r"(\d{8})")
        .unwrap()
        .find(&file_name)
        .map(|m| m.as_str().to_string())
}

fn discover_latest_rules_url() -> Result<String> {
    let html = reqwest::blocking::get(RULES_INDEX_URL)
        .context("téléchargement de la page des règles Wizards")?
        .text()
        .context("lecture de la page des règles Wizards")?;
    extract_rules_url(&html).ok_or_else(|| {
        anyhow::anyhow!(
            "impossible de trouver l'URL du fichier .txt des règles sur {RULES_INDEX_URL}"
        )
    })
}

/// Premier lien `.txt` de `media.wizards.com` de la page, espaces encodés en `%20`.
/// Le nom de fichier peut contenir des espaces littéraux ; guillemets, `<`, `>` et
/// retours à la ligne et tabulations bornent le lien.
fn extract_rules_url(html: &str) -> Option<String> {
    static RULES_URL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"https://media\.wizards\.com/[^"'<>\r\n\t]+?\.txt"#).unwrap()
    });
    RULES_URL.find(html).map(|m| m.as_str().replace(' ', "%20"))
}

fn atomic_swap(tmp_path: &Path, final_path: &Path) -> Result<()> {
    std::fs::rename(tmp_path, final_path)
        .with_context(|| format!("remplacement atomique de {}", final_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENCODED: &str = "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt";

    #[test]
    fn extracts_link_with_literal_space() {
        let html = r#"<a href="https://media.wizards.com/2026/downloads/MagicCompRules 20260925.txt" class="x">Règles</a>"#;
        let url = extract_rules_url(html).unwrap();
        assert_eq!(url, ENCODED);
        assert_eq!(version_from_url(&url).as_deref(), Some("20260925"));
    }

    #[test]
    fn already_encoded_link_is_not_double_encoded() {
        let html = format!(r#"<a href="{ENCODED}">Règles</a>"#);
        let url = extract_rules_url(&html).unwrap();
        assert_eq!(url, ENCODED);
        assert_eq!(version_from_url(&url).as_deref(), Some("20260925"));
    }

    #[test]
    fn extracts_link_from_escaped_json_string() {
        let html = r#"{\"href\":\"https://media.wizards.com/2026/downloads/MagicCompRules 20260925.txt\",\"t\":1}"#;
        // Le lien s'arrête au `.txt` ; le `\` d'échappement n'en fait pas partie.
        assert_eq!(extract_rules_url(html).as_deref(), Some(ENCODED));
    }

    #[test]
    fn link_stops_at_txt_before_following_attributes() {
        let html = r#"<a href="https://media.wizards.com/2026/downloads/MagicCompRules 20260925.txt" title="a b.txt" target="_blank">x</a>"#;
        assert_eq!(extract_rules_url(html).as_deref(), Some(ENCODED));
    }

    #[test]
    fn no_txt_link_returns_none() {
        let html = "<a href=\"https://media.wizards.com/2026/downloads/rules.pdf\">PDF</a>\n\
<a href=\"https://example.com/rules.txt\">autre</a>\n\
https://media.wizards.com/2026/downloads/Magic\tRules.txt";
        assert_eq!(extract_rules_url(html), None);
    }
}
