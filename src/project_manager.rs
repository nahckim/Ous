use serde_json::Value;
use std::fs;

pub fn load_projects() -> Vec<Value> {
    match fs::read_to_string("data/lytho_cache.json") {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|_| vec![]),
        Err(_) => vec![],
    }
}