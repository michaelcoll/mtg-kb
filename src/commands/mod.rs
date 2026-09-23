pub mod analyze;
pub mod card;
pub mod overrides;
pub mod report;
pub mod rules;
pub mod rulings;
pub mod search;
pub mod set;
pub mod update;
pub mod update_rules;

use anyhow::Result;

use crate::data_dir::{cards_db_path, overrides_db_path};
use crate::db::cards::CardsDb;
use crate::db::overrides::load_corrections;

/// Base cartes avec les Corrections de `overrides.sqlite` (ADR 0005).
fn open_cards_db() -> Result<CardsDb> {
    Ok(CardsDb::open(&cards_db_path())?.with_corrections(load_corrections(&overrides_db_path())?))
}
