use std::env;
use std::path::PathBuf;

const KB_DATA_DIR_VAR: &str = "KB_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";
const CARDS_DB_FILE: &str = "AllPrintings.sqlite";
const RULES_DB_FILE: &str = "rules.sqlite";
const OVERRIDES_DB_FILE: &str = "overrides.sqlite";
const CACHE_DIR: &str = "cache";
const EDHREC_CACHE_SUBDIR: &str = "edhrec";

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

pub fn overrides_db_path() -> PathBuf {
    data_dir().join(OVERRIDES_DB_FILE)
}

pub fn cache_dir() -> PathBuf {
    data_dir().join(CACHE_DIR)
}

pub fn edhrec_cache_dir() -> PathBuf {
    cache_dir().join(EDHREC_CACHE_SUBDIR)
}
