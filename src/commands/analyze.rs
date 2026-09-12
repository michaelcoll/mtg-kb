use std::io::Read;

use anyhow::{Context, Result};

use crate::analyze;
use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::output::print_json;

pub fn run(source: &str) -> Result<()> {
    let input = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de la Decklist depuis l'entrée standard")?;
        buf
    } else {
        std::fs::read_to_string(source).with_context(|| format!("lecture du fichier {source}"))?
    };

    let db = CardsDb::open(&cards_db_path())?;
    let result = analyze::run(&input, &db)?;
    print_json(&result)
}
