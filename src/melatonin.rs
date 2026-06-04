use super::memory_manager::MemoryManager;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::{Utc, Duration};
use serde_json::{json, Value};

pub async fn run_melatonin(memory_manager: Arc<MemoryManager>) {
    println!("[Melatonin] Starting...");
    let now = Utc::now();
    let yesterday = now - Duration::days(1);

    let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
    let mut approved_entries = Vec::new();
    let mut decision_entries = Vec::new();

    if let Ok(entries) = memory_manager.read_entries() {
        for entry in entries {
            if let Some(ts) = entry["timestamp"].as_str() {
                if let Ok(ts_parsed) = chrono::DateTime::parse_from_rfc3339(ts) {
                    let ts_utc = ts_parsed.with_timezone(&Utc);
                    if ts_utc >= yesterday {
                        let entity_type = entry["entity_type"].as_str().unwrap_or("unknown");
                        grouped.entry(entity_type.to_string()).or_insert_with(Vec::new).push(entry.clone());

                        if entry["approved_by_user"].as_bool().unwrap_or(false) {
                            approved_entries.push(entry.clone());
                        }

                        if let Some(after_state) = entry.get("after_state") {
                            if after_state.get("decision").is_some() || after_state.get("error").is_some() {
                                decision_entries.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    let output = json!({
        "generated_at": now.to_rfc3339(),
        "period": {
            "start": yesterday.to_rfc3339(),
            "end": now.to_rfc3339()
        },
        "grouped_by_type": grouped,
        "approved_count": approved_entries.len(),
        "approved_entries": approved_entries,
        "decision_entries": decision_entries
    });

    if let Ok(json_str) = serde_json::to_string_pretty(&output) {
        match std::fs::write("data/memory/melatonin_staging.json", json_str) {
            Ok(_) => println!("[Melatonin] Staging file written"),
            Err(e) => eprintln!("[Melatonin] Failed to write staging: {}", e)
        }
    }
}
