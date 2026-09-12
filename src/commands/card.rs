use anyhow::{Result, bail};

use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::model::Card;
use crate::output::{Format, print_json};

pub fn run(name: &str, format: Format) -> Result<()> {
    let db = CardsDb::open(&cards_db_path())?;
    let Some(card) = db.card_by_name(name)? else {
        bail!("Carte introuvable : « {} »", name);
    };
    match format {
        Format::Json => print_json(&card)?,
        Format::Table => print_table(&card),
    }
    Ok(())
}

fn print_table(card: &Card) {
    let rows: Vec<(&str, String)> = vec![
        ("name", card.name.clone()),
        ("mana_cost", card.mana_cost.clone().unwrap_or_default()),
        (
            "mana_value",
            card.mana_value.map(|v| v.to_string()).unwrap_or_default(),
        ),
        ("type_line", card.type_line.clone().unwrap_or_default()),
        ("color_identity", card.color_identity.join(", ")),
        ("keywords", card.keywords.join(", ")),
        ("power", card.power.clone().unwrap_or_default()),
        ("toughness", card.toughness.clone().unwrap_or_default()),
        ("loyalty", card.loyalty.clone().unwrap_or_default()),
        ("oracle_text", card.oracle_text.clone().unwrap_or_default()),
    ];
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, value) in rows {
        println!("{:width$}  {}", key, value, width = width);
    }
}
