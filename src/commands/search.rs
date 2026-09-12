use anyhow::Result;

use crate::data_dir::cards_db_path;
use crate::db::cards::{CardsDb, SearchFilters};
use crate::model::Card;
use crate::output::{Format, print_json};

#[allow(clippy::too_many_arguments)]
pub fn run(
    name: Option<String>,
    type_contains: Option<String>,
    text: Option<String>,
    color_identity: Option<String>,
    legal_in: Option<String>,
    mana_value: Option<f64>,
    limit: usize,
    format: Format,
) -> Result<()> {
    let db = CardsDb::open(&cards_db_path())?;
    let filters = SearchFilters {
        name,
        type_contains,
        oracle_text_contains: text,
        color_identity_subset_of: color_identity
            .map(|s| s.chars().map(|c| c.to_string()).collect()),
        legal_in_format: legal_in,
        mana_value,
        limit,
    };
    let results = db.search(&filters)?;
    match format {
        Format::Json => print_json(&results)?,
        Format::Table => print_table(&results),
    }
    Ok(())
}

fn print_table(cards: &[Card]) {
    for card in cards {
        println!(
            "{:<30} {:<6} {}",
            card.name,
            card.mana_cost.clone().unwrap_or_default(),
            card.type_line.clone().unwrap_or_default()
        );
    }
}
