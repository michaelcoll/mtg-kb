use std::env;
use std::path::PathBuf;

const KB_DATA_DIR_VAR: &str = "KB_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";
const CARDS_DB_FILE: &str = "AllPrintings.sqlite";
const RULES_DB_FILE: &str = "rules.sqlite";

pub fn data_dir() -> PathBuf {
    match env::var_os(KB_DATA_DIR_VAR) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(DEFAULT_DATA_DIR),
    }
}

pub fn cards_db_path() -> PathBuf {
    data_dir().join(CARDS_DB_FILE)
}

pub fn rules_db_path() -> PathBuf {
    data_dir().join(RULES_DB_FILE)
}
