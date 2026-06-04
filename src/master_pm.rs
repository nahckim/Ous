use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use std::sync::Arc;

pub async fn run_master_pm(bus: Arc<MessageBus>, memory_manager: Arc<MemoryManager>) {
    let mut rx = bus.subscribe();
    loop {
        if let Ok(msg) = rx.recv().await {
            if let Some(entry_id) = msg.strip_prefix("memory:new_capture:") {
                println!("[MasterPM] New capture: {}", entry_id);
                if let Ok(entries) = memory_manager.read_entries() {
                    if let Some(entry) = entries.iter().find(|e| e["entity_id"].as_str() == Some(entry_id)) {
                        let content = entry["after_state"]["content"].as_str().unwrap_or("");
                        let content_lower = content.to_lowercase();
                        let category = if content_lower.contains("lytho") || content_lower.contains("resorts")
                            || content_lower.contains("project") || content_lower.contains("deadline")
                            || content_lower.contains("csv") {
                            "work"
                        } else if content_lower.contains("ous") || content_lower.contains("workspace")
                            || content_lower.contains("bid") || content_lower.contains("sensor")
                            || content_lower.contains("rust") {
                            "os"
                        } else {
                            "life"
                        };
                        println!("[MasterPM] {} -> {}", entry_id, category);
                    }
                }
            }
        }
    }
}
