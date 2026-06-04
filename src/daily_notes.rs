use super::memory_manager::MemoryManager;
use std::sync::Arc;
use chrono::{Utc, Timelike, Duration, NaiveTime};
use tokio::time::{interval, Duration as TokioDuration};
use std::fs;
use std::path::PathBuf;

pub async fn run_daily_writer(memory_manager: Arc<MemoryManager>) {
    let mut interval = interval(TokioDuration::from_secs(3600));
    loop {
        interval.tick().await;
        let now = Utc::now();
        if now.hour() == 0 && now.minute() < 5 {
            let yesterday = now - Duration::days(1);
            let date_str = yesterday.format("%Y-%m-%d").to_string();
            let output_path = PathBuf::from("data/memory/DAILY").join(format!("{}.md", date_str));
            if !output_path.exists() {
                if let Ok(entries) = memory_manager.read_entries() {
                    let mut md = String::new();
                    md.push_str(&format!("# Daily Notes - {}\n\n", date_str));
                    let mut captures = Vec::new();
                    let mut decisions = Vec::new();
                    let mut projects = Vec::new();
                    let mut errors = Vec::new();
                    let day_start = yesterday.with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()).unwrap();
                    let day_end = yesterday.with_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap()).unwrap();
                    for entry in entries {
                        if let Some(ts) = entry["timestamp"].as_str() {
                            if let Ok(ts_parsed) = chrono::DateTime::parse_from_rfc3339(ts) {
                                let ts_utc = ts_parsed.with_timezone(&Utc);
                                if ts_utc >= day_start && ts_utc <= day_end {
                                    let etype = entry["entity_type"].as_str().unwrap_or("");
                                    match etype {
                                        "capture" => captures.push(entry),
                                        "decision" => decisions.push(entry),
                                        "project" => projects.push(entry),
                                        "error" => errors.push(entry),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    md.push_str(&format!("## Captures ({})\n\n", captures.len()));
                    for cap in captures {
                        let content = cap["after_state"]["content"].as_str().unwrap_or("");
                        md.push_str(&format!("- {}\n", content));
                    }
                    md.push_str("\n## Decisions\n\n");
                    for dec in decisions {
                        let content = dec["after_state"]["summary"].as_str().unwrap_or("");
                        md.push_str(&format!("- {}\n", content));
                    }
                    md.push_str("\n## Projects\n\n");
                    for proj in projects {
                        let name = proj["after_state"]["name"].as_str().unwrap_or("");
                        let status = proj["after_state"]["status"].as_str().unwrap_or("");
                        md.push_str(&format!("- {}: {}\n", name, status));
                    }
                    md.push_str("\n## Errors\n\n");
                    for err in errors {
                        let content = err["after_state"]["content"].as_str().unwrap_or("");
                        md.push_str(&format!("- {}\n", content));
                    }
                    let _ = fs::create_dir_all(output_path.parent().unwrap());
                    let _ = fs::write(&output_path, md);
                    println!("[DailyNotes] Wrote {}", output_path.display());
                }
            }
        }
    }
}
