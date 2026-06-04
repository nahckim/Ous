use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use super::ai_router::AIExecutor;
use super::approval;
use std::sync::Arc;
use chrono::{Utc, Duration};
use std::fs;

pub async fn run_dreaming(bus: Arc<MessageBus>, memory_manager: Arc<MemoryManager>, ai_executor: Arc<AIExecutor>) {
    let mut rx = bus.subscribe();
    loop {
        if let Ok(msg) = rx.recv().await {
            if msg == "dream:trigger" {
                println!("[Dreaming] Starting dream cycle...");
                let now = Utc::now();
                let yesterday = now - Duration::days(1);
                let mut recent_entries = Vec::new();
                if let Ok(entries) = memory_manager.read_entries() {
                    for entry in entries {
                        if let Some(ts) = entry["timestamp"].as_str() {
                            if let Ok(ts_parsed) = chrono::DateTime::parse_from_rfc3339(ts) {
                                let ts_utc = ts_parsed.with_timezone(&Utc);
                                if ts_utc >= yesterday {
                                    recent_entries.push(entry);
                                }
                            }
                        }
                    }
                }
                let memory_path = "data/memory/MEMORY.md";
                let existing_memory = fs::read_to_string(memory_path).unwrap_or_default();
                let recent_json = serde_json::to_string_pretty(&recent_entries).unwrap_or_default();
                let prompt = format!(
                    "You are Ous, a cognitive OS. Review the following recent journal entries and the existing MEMORY.md. Propose updates to MEMORY.md (add/update/remove) as a JSON list of actions. Each action: {{\"action\":\"add\",\"section\":\"...\",\"content\":\"...\"}} or {{\"action\":\"update\",\"section\":\"...\",\"old\":\"...\",\"new\":\"...\"}}. Output ONLY valid JSON.\n\nRecent entries:\n{}\n\nExisting MEMORY.md:\n{}\n",
                    recent_json, existing_memory
                );
                match ai_executor.execute("summarize", &prompt).await {
                    Ok(ai_output) => {
                        let cleaned = ai_output.trim();
                        let json_str = if cleaned.contains("```json") {
                            cleaned.split("```json").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                        } else if cleaned.contains("```") {
                            cleaned.split("```").nth(1).and_then(|s| s.split("```").next()).unwrap_or(cleaned)
                        } else {
                            cleaned
                        };
                        if let Ok(proposals) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                            println!("\n[Dreaming] Proposed updates to MEMORY.md:");
                            for (i, prop) in proposals.iter().enumerate() {
                                println!("{}. {}", i+1, serde_json::to_string_pretty(prop).unwrap());
                            }
                            if approval::request_approval("Apply these memory updates?").await {
                                let mut new_memory = existing_memory;
                                for prop in proposals {
                                    if let Some(action) = prop["action"].as_str() {
                                        if action == "add" {
                                            let section = prop["section"].as_str().unwrap_or("");
                                            let content = prop["content"].as_str().unwrap_or("");
                                            new_memory.push_str(&format!("\n## {}\n{}\n", section, content));
                                        }
                                    }
                                }
                                if let Err(e) = fs::write(memory_path, new_memory) {
                                    eprintln!("[Dreaming] Failed to write MEMORY.md: {}", e);
                                } else {
                                    println!("[Dreaming] MEMORY.md updated.");
                                }
                            } else {
                                println!("[Dreaming] Updates rejected.");
                            }
                        } else {
                            eprintln!("[Dreaming] Invalid JSON from AI: {}", ai_output);
                        }
                    }
                    Err(e) => eprintln!("[Dreaming] AI error: {}", e),
                }
            }
        }
    }
}
