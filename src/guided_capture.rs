use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use super::ai_router::AIExecutor;
use super::approval;
use std::sync::Arc;
use std::path::PathBuf;
use serde::Deserialize;
use tokio::fs;
use tokio::time::{interval, Duration};
use serde_json::json;
use chrono::Utc;

#[derive(Debug, Deserialize)]
struct GuidedPacket {
    id: String,
    prompt: String,
    #[serde(default)]
    _context_hint: Option<String>,
    require_approval: bool,
}

pub async fn watch_guided_folder(
    _bus: Arc<MessageBus>,
    memory_manager: Arc<MemoryManager>,
    ai_executor: Arc<AIExecutor>,
    path: &str
) {
    let mut interval = interval(Duration::from_secs(10));
    let mut processed = std::collections::HashSet::new();
    loop {
        interval.tick().await;
        if let Ok(mut entries) = fs::read_dir(path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.ends_with(".json") && file_name.starts_with("guided_") && !processed.contains(&file_name) {
                    let file_path = entry.path();
                    match fs::read_to_string(&file_path).await {
                        Ok(content) => {
                            match serde_json::from_str::<GuidedPacket>(&content) {
                                Ok(packet) => {
                                    println!("[GUIDED] Processing: {}", packet.id);
                                    let ollama_prompt = format!(
                                        "Classify this note into entity_type, entity_id, and after_state.summary. Respond with ONLY a JSON object, no markdown, no explanation. entity_type must be one of: capture, decision, project, error. entity_id must be a snake_case identifier with no dots or ellipsis. after_state.summary must be one sentence. Note: {}. Example output: {{\"entity_type\":\"decision\",\"entity_id\":\"use_ollama_local\",\"after_state\":{{\"summary\":\"Chose Ollama for local inference to reduce cost.\"}}}}",
                                        packet.prompt
                                    );
                                    match ai_executor.execute("summarize", &ollama_prompt).await {
                                        Ok(ai_output) => {
                                            let cleaned = ai_output.trim();
                                            let json_str = if cleaned.contains("```json") {
                                                cleaned.split("```json").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                                            } else if cleaned.contains("```") {
                                                cleaned.split("```").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                                            } else {
                                                cleaned
                                            };
                                            if let Ok(mut entry) = serde_json::from_str::<serde_json::Value>(json_str) {
                                                entry["entry_id"] = json!(uuid::Uuid::new_v4().to_string());
                                                entry["timestamp"] = json!(Utc::now().to_rfc3339());
                                                entry["schema_version"] = json!(1);
                                                entry["reason_capture_id"] = json!(null);
                                                entry["approved_by_user"] = json!(false);
                                                if packet.require_approval {
                                                    println!("\n[GUIDED] Proposed journal entry:");
                                                    println!("{}", serde_json::to_string_pretty(&entry).unwrap());
                                                    if approval::request_approval("Approve this entry?").await {
                                                        if let Err(e) = memory_manager.append(entry) {
                                                            eprintln!("[GUIDED] Failed to write: {}", e);
                                                        } else {
                                                            println!("[GUIDED] Entry written to journal.");
                                                        }
                                                    } else {
                                                        println!("[GUIDED] Entry rejected.");
                                                    }
                                                } else {
                                                    if let Err(e) = memory_manager.append(entry) {
                                                        eprintln!("[GUIDED] Failed to write: {}", e);
                                                    } else {
                                                        println!("[GUIDED] Entry written (auto-approved).");
                                                    }
                                                }
                                            } else {
                                                eprintln!("[GUIDED] AI output not valid JSON, falling back to manual.");
                                                manual_entry(&memory_manager, &packet).await;
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[GUIDED] AI error: {}, falling back to manual.", e);
                                            manual_entry(&memory_manager, &packet).await;
                                        }
                                    }
                                    processed.insert(file_name.clone());
                                    let archive_dir = PathBuf::from(path).join("archive");
                                    let _ = fs::create_dir_all(&archive_dir).await;
                                    let dest = archive_dir.join(&file_name);
                                    if let Err(e) = fs::rename(&file_path, &dest).await {
                                        eprintln!("[GUIDED] Failed to archive {}: {}", file_name, e);
                                    } else {
                                        println!("[GUIDED] Archived: {}", file_name);
                                    }
                                }
                                Err(e) => eprintln!("[GUIDED] JSON parse error: {}", e),
                            }
                        }
                        Err(e) => eprintln!("[GUIDED] Cannot read: {}", e),
                    }
                }
            }
        }
    }
}

async fn manual_entry(memory_manager: &Arc<MemoryManager>, packet: &GuidedPacket) {
    println!("[GUIDED] Manual entry mode for: {}", packet.prompt);
    println!("entity_type (capture/decision/project): ");
    let mut entity_type = String::new();
    std::io::stdin().read_line(&mut entity_type).unwrap();
    let entity_type = entity_type.trim().to_lowercase();
    println!("entity_id (short name, no spaces): ");
    let mut entity_id = String::new();
    std::io::stdin().read_line(&mut entity_id).unwrap();
    let entity_id = entity_id.trim();
    println!("summary (one sentence): ");
    let mut summary = String::new();
    std::io::stdin().read_line(&mut summary).unwrap();
    let summary = summary.trim();
    let entry = json!({
        "entry_id": uuid::Uuid::new_v4().to_string(),
        "timestamp": Utc::now().to_rfc3339(),
        "schema_version": 1,
        "entity_type": entity_type,
        "entity_id": entity_id,
        "operation": "create",
        "before_state": null,
        "after_state": { "summary": summary, "original_prompt": packet.prompt },
        "reason_capture_id": null,
        "approved_by_user": false
    });
    if packet.require_approval {
        println!("\n[GUIDED] Proposed journal entry:");
        println!("{}", serde_json::to_string_pretty(&entry).unwrap());
        if approval::request_approval("Approve this entry?").await {
            if let Err(e) = memory_manager.append(entry) {
                eprintln!("[GUIDED] Failed to write: {}", e);
            } else {
                println!("[GUIDED] Entry written to journal.");
            }
        } else {
            println!("[GUIDED] Entry rejected.");
        }
    } else {
        if let Err(e) = memory_manager.append(entry) {
            eprintln!("[GUIDED] Failed to write: {}", e);
        } else {
            println!("[GUIDED] Entry written (auto-approved).");
        }
    }
}

