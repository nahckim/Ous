use serde_json::Value;
use std::fs;

pub fn _load_projects() -> Vec<Value> {
    fs::read_to_string("data/lytho_cache.json")
        .map(|s| serde_json::from_str(&s).unwrap_or_default())
        .unwrap_or_default()
}
