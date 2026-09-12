use anyhow::{Result, bail};

use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::model::Ruling;
use crate::output::{Format, print_json};

pub fn run(name: &str, format: Format) -> Result<()> {
    let db = CardsDb::open(&cards_db_path())?;
    let Some(rulings) = db.rulings_by_name(name)? else {
        bail!("Carte introuvable : « {} »", name);
    };
    match format {
        Format::Json => print_json(&rulings)?,
        Format::Table => print_table(&rulings),
    }
    Ok(())
}

fn print_table(rulings: &[Ruling]) {
    for ruling in rulings {
        println!("{}  {}", ruling.date, ruling.text);
    }
}
