use std::io::Read;

use anyhow::{Context, Result};

use crate::analyze;
use crate::analyze::metrics::Thresholds;
use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::output::print_json;

#[allow(clippy::too_many_arguments)]
pub fn run(
    source: &str,
    min_lands: u32,
    min_ramp: u32,
    min_draw: u32,
    min_removal: u32,
    min_wipe: u32,
    max_average_mana_value: f64,
    max_high_cost_cards: u32,
) -> Result<()> {
    let input = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de la Decklist depuis l'entrée standard")?;
        buf
    } else {
        std::fs::read_to_string(source).with_context(|| format!("lecture du fichier {source}"))?
    };

    let thresholds = Thresholds {
        min_lands,
        min_ramp,
        min_draw,
        min_removal,
        min_wipe,
        max_average_mana_value,
        max_high_cost_cards,
    };

    let db = CardsDb::open(&cards_db_path())?;
    let result = analyze::run(&input, &db, &thresholds)?;
    print_json(&result)
}
