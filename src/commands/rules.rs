use anyhow::{Result, bail};
use clap::ValueEnum;

use crate::data_dir::rules_db_path;
use crate::db::rules::RulesDb;
use crate::model::{GlossaryDefinition, RuleEntryOut, RuleWithChildren};
use crate::output::{Format, print_json};

/// `kb rules <numéro> [--format table]`, capturé via un external_subcommand
/// clap car un numéro de Règle n'est ni "search" ni "define".
pub fn dispatch_number(args: &[String]) -> Result<()> {
    let Some(number) = args.first() else {
        bail!("usage: kb rules <numéro> [--format table|json]");
    };
    let mut format = Format::Json;
    if let Some(pos) = args.iter().position(|a| a == "--format")
        && let Some(value) = args.get(pos + 1)
    {
        format = Format::from_str(value, true)
            .map_err(|e| anyhow::anyhow!("--format invalide : {e}"))?;
    }
    show(number, format)
}

pub fn show(number: &str, format: Format) -> Result<()> {
    let db = RulesDb::open(&rules_db_path())?;
    let Some(rule) = db.rule_by_number(number)? else {
        bail!("Règle introuvable : « {} »", number);
    };
    match format {
        Format::Json => print_json(&rule)?,
        Format::Table => print_rule_table(&rule),
    }
    Ok(())
}

pub fn search(query: &str, limit: usize, format: Format) -> Result<()> {
    let db = RulesDb::open(&rules_db_path())?;
    let results = db.search(query, limit)?;
    match format {
        Format::Json => print_json(&results)?,
        Format::Table => print_entries_table(&results),
    }
    Ok(())
}

pub fn define(term: &str, format: Format) -> Result<()> {
    let db = RulesDb::open(&rules_db_path())?;
    let Some(definition) = db.define(term)? else {
        bail!("Terme introuvable dans le Glossaire : « {} »", term);
    };
    match format {
        Format::Json => print_json(&definition)?,
        Format::Table => print_glossary_table(&definition),
    }
    Ok(())
}

fn print_rule_table(rule: &RuleWithChildren) {
    if let Some(title) = &rule.title {
        println!("{}  {}", rule.number, title);
    }
    if let Some(text) = &rule.text {
        println!("{}  {}", rule.number, text);
    }
    for child in &rule.children {
        println!("{}  {}", child.number, child.text);
    }
}

fn print_entries_table(entries: &[RuleEntryOut]) {
    for entry in entries {
        println!("{}  {}", entry.number, entry.text);
    }
}

fn print_glossary_table(definition: &GlossaryDefinition) {
    println!("{}\n{}", definition.term, definition.definition);
}
