use super::memory_manager::MemoryManager;
use std::sync::Arc;
use chrono::{Utc, Duration};
use tokio::time::{interval, Duration as TokioDuration};
use std::fs;
use std::path::PathBuf;
use std::io::Write;
use serde_json::Value;

pub async fn run_pruner(memory_manager: Arc<MemoryManager>) {
    let mut interval = interval(TokioDuration::from_secs(86400));
    loop {
        interval.tick().await;
        if let Ok(entries) = memory_manager.read_entries() {
            let now = Utc::now();
            let thirty_days_ago = now - Duration::days(30);
            let mut to_archive = Vec::new();
            let mut keep = Vec::new();
            for entry in entries {
                if let Some(ts) = entry["timestamp"].as_str() {
                    if let Ok(ts_parsed) = chrono::DateTime::parse_from_rfc3339(ts) {
                        let ts_utc = ts_parsed.with_timezone(&Utc);
                        if ts_utc < thirty_days_ago {
                            let approval = entry["approved_by_user"].as_bool().unwrap_or(false);
                            let score = if approval { 0.5 } else { 0.0 };
                            if score < 0.4 {
                                to_archive.push(entry);
                                continue;
                            }
                        }
                    }
                }
                keep.push(entry);
            }
            if !to_archive.is_empty() {
                let minor_path = PathBuf::from("data/memory/MINOR_LEAGUE.jsonl");
                let _ = fs::create_dir_all(minor_path.parent().unwrap());
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&minor_path)
                    .unwrap();
                for entry in &to_archive {
                    let line = serde_json::to_string(entry).unwrap();
                    let _ = writeln!(file, "{}", line);
                }
                for entry in &to_archive {
                    let original_id = entry["entry_id"].as_str().unwrap_or("");
                    let tombstone = memory_manager.new_archived_entry(original_id);
                    let _ = memory_manager.append(tombstone);
                }
                let keep_filtered: Vec<Value> = keep.iter().filter(|e| !e.is_null()).cloned().collect();
                let _ = memory_manager.rewrite_entries(&keep_filtered);
                println!("[MinorLeague] Archived {} entries", to_archive.len());
            }
        }
    }
}
