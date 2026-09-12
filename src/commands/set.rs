use anyhow::{Result, bail};

use crate::data_dir::cards_db_path;
use crate::db::cards::CardsDb;
use crate::model::SetInfo;
use crate::output::{Format, print_json};

pub fn run(code: &str, format: Format) -> Result<()> {
    let db = CardsDb::open(&cards_db_path())?;
    let Some(set) = db.set_by_code(code)? else {
        bail!("Set introuvable : « {} »", code);
    };
    match format {
        Format::Json => print_json(&set)?,
        Format::Table => print_table(&set),
    }
    Ok(())
}

fn print_table(set: &SetInfo) {
    println!("code             {}", set.code);
    println!("name             {}", set.name);
    println!(
        "release_date     {}",
        set.release_date.clone().unwrap_or_default()
    );
    println!(
        "type             {}",
        set.set_type.clone().unwrap_or_default()
    );
    println!("block            {}", set.block.clone().unwrap_or_default());
    println!(
        "base_set_size    {}",
        set.base_set_size.map(|v| v.to_string()).unwrap_or_default()
    );
    println!(
        "total_set_size   {}",
        set.total_set_size
            .map(|v| v.to_string())
            .unwrap_or_default()
    );
}
