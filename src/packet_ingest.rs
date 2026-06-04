use tokio::fs;
use tokio::time::{interval, Duration};
use std::path::PathBuf;
use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use super::ai_router::AIExecutor;
use super::approval;
use std::sync::Arc;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Packet {
    Regular { id: String, task: String, prompt: String, require_approval: Option<bool> },
    Project { name: String, context_hint: Option<String>, require_approval: Option<bool> },
    ProjectUpdate { name: String, status: String, require_approval: Option<bool> },
    Dream { require_approval: Option<bool> },
    SelfEdit { prompt: String, require_approval: Option<bool> },
}

pub async fn watch_packets_folder(bus: Arc<MessageBus>, memory_manager: Arc<MemoryManager>, _ai_executor: Arc<AIExecutor>, path: &str) {
    let mut interval = interval(Duration::from_secs(10));
    loop {
        interval.tick().await;
        if let Ok(mut entries) = fs::read_dir(path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.ends_with(".json") && !file_name.contains("archive") {
                    let file_path = entry.path();
                    match fs::read_to_string(&file_path).await {
                        Ok(content) => {
                            let process_result = match serde_json::from_str::<Packet>(&content) {
                                Ok(Packet::Regular { id, task, prompt, require_approval }) => {
                                    println!("[PACKET] Regular packet: {} (task: {})", id, task);
                                    let entry = memory_manager.new_capture_entry(&id, &prompt, "packet");
                                    if let Err(e) = memory_manager.append(entry) {
                                        eprintln!("[PACKET] Failed to write to journal: {}", e);
                                    } else {
                                        bus.publish("memory:new_capture", &id);
                                    }
                                    let payload = format!("{}|{}", task, prompt);
                                    bus.publish("ai:run", &payload);
                                    if require_approval.unwrap_or(false) {
                                        bus.publish("suggestion", &format!("approve_packet:{}", id));
                                    }
                                    Ok(())
                                }
                                Ok(Packet::Project { name, context_hint, require_approval }) => {
                                    let category = context_hint.as_deref().unwrap_or("work");
                                    println!("[PACKET] Project packet: {} (category: {})", name, category);
                                    let mut entry = memory_manager.new_project_entry(&name, category, require_approval.unwrap_or(true));
                                    let approve = require_approval.unwrap_or(true);
                                    if approve {
                                        println!("\n[PACKET] Proposed project entry:");
                                        println!("{}", serde_json::to_string_pretty(&entry).unwrap());
                                        if approval::request_approval("Create this project?").await {
                                            if let Some(obj) = entry.as_object_mut() {
                                                obj.insert("approved_by_user".to_string(), json!(true));
                                            }
                                            if let Err(e) = memory_manager.append(entry) {
                                                eprintln!("[PACKET] Failed to write project: {}", e);
                                            } else {
                                                bus.publish("memory:new_capture", &name);
                                                println!("[PACKET] Project created and journal updated.");
                                            }
                                        } else {
                                            println!("[PACKET] Project creation rejected.");
                                        }
                                    } else {
                                        if let Err(e) = memory_manager.append(entry) {
                                            eprintln!("[PACKET] Failed to write project: {}", e);
                                        } else {
                                            bus.publish("memory:new_capture", &name);
                                            println!("[PACKET] Project created (auto-approved).");
                                        }
                                    }
                                    Ok(())
                                }
                                Ok(Packet::ProjectUpdate { name, status, require_approval }) => {
                                    println!("[PACKET] Project update: {} -> {}", name, status);
                                    let mut old_status = String::new();
                                    let mut category = String::new();
                                    if let Ok(entries) = memory_manager.read_entries() {
                                        for entry in entries {
                                            if entry["entity_type"].as_str() == Some("project") && entry["after_state"]["name"].as_str() == Some(&name) {
                                                old_status = entry["after_state"]["status"].as_str().unwrap_or("").to_string();
                                                category = entry["after_state"]["category"].as_str().unwrap_or("").to_string();
                                                break;
                                            }
                                        }
                                    }
                                    let update_entry = memory_manager.new_project_update_entry(&name, &old_status, &status, &category);
                                    let approve = require_approval.unwrap_or(true);
                                    if approve {
                                        println!("\n[PACKET] Proposed project update:");
                                        println!("{}", serde_json::to_string_pretty(&update_entry).unwrap());
                                        if approval::request_approval("Apply this update?").await {
                                            if let Err(e) = memory_manager.append(update_entry) {
                                                eprintln!("[PACKET] Failed to write update: {}", e);
                                            } else {
                                                bus.publish("memory:new_capture", &format!("project_update:{}", name));
                                                println!("[PACKET] Project updated.");
                                            }
                                        } else {
                                            println!("[PACKET] Update rejected.");
                                        }
                                    } else {
                                        if let Err(e) = memory_manager.append(update_entry) {
                                            eprintln!("[PACKET] Failed to write update: {}", e);
                                        } else {
                                            bus.publish("memory:new_capture", &format!("project_update:{}", name));
                                            println!("[PACKET] Project updated (auto-approved).");
                                        }
                                    }
                                    Ok(())
                                }
                                Ok(Packet::Dream { require_approval }) => {
                                    println!("[PACKET] Dream packet received");
                                    if require_approval.unwrap_or(true) {
                                        if approval::request_approval("Trigger dreaming?").await {
                                            bus.publish("dream:trigger", "");
                                            println!("[PACKET] Dreaming triggered.");
                                        } else {
                                            println!("[PACKET] Dreaming rejected.");
                                        }
                                    } else {
                                        bus.publish("dream:trigger", "");
                                        println!("[PACKET] Dreaming triggered (auto-approved).");
                                    }
                                    Ok(())
                                }
                                Ok(Packet::SelfEdit { prompt, require_approval }) => {
                                    println!("[PACKET] Self-edit packet: {}", &prompt[..prompt.len().min(50)]);
                                    match _ai_executor.execute("self_edit", &prompt).await {
                                        Ok(ai_output) => {
                                            let cleaned = ai_output.trim();
                                            let code = if cleaned.contains("```rust") {
                                                cleaned.split("```rust").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                                            } else if cleaned.contains("```") {
                                                cleaned.split("```").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                                            } else {
                                                cleaned
                                            };
                                            println!("\n[PACKET] Proposed main.rs update:\n---\n{}\n---", &code[..code.len().min(500)]);
                                            if require_approval.unwrap_or(true) && approval::request_approval("Apply this change?").await {
                                                match std::fs::write("src/main.rs", code) {
                                                    Ok(_) => println!("[PACKET] main.rs updated. Restart Ous to apply."),
                                                    Err(e) => eprintln!("[PACKET] Failed to write main.rs: {}", e),
                                                }
                                            } else if !require_approval.unwrap_or(true) {
                                                match std::fs::write("src/main.rs", code) {
                                                    Ok(_) => println!("[PACKET] main.rs updated (auto-approved). Restart Ous to apply."),
                                                    Err(e) => eprintln!("[PACKET] Failed to write main.rs: {}", e),
                                                }
                                            } else {
                                                println!("[PACKET] Self-edit rejected.");
                                            }
                                        }
                                        Err(e) => eprintln!("[PACKET] AI error on self-edit: {}", e),
                                    }
                                    Ok(())
                                }
                                Err(e) => {
                                    eprintln!("[PACKET] JSON parse error: {}", e);
                                    Err(())
                                }
                            };
                            if process_result.is_ok() {
                                let archive_dir = PathBuf::from(path).join("archive");
                                let _ = fs::create_dir_all(&archive_dir).await;
                                let dest = archive_dir.join(&file_name);
                                if let Err(e) = fs::rename(&file_path, &dest).await {
                                    eprintln!("[PACKET] Failed to archive {}: {}", file_name, e);
                                } else {
                                    println!("[PACKET] Archived: {}", file_name);
                                }
                            }
                        }
                        Err(e) => eprintln!("[PACKET] Cannot read {}: {}", file_path.display(), e),
                    }
                }
            }
        }
    }
}
