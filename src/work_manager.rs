use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use std::sync::Arc;
use std::collections::HashMap;
use serde_json::Value;
use super::dashboard::SharedProjectMap;

pub async fn run_work_manager(bus: Arc<MessageBus>, memory_manager: Arc<MemoryManager>, projects: SharedProjectMap) {
    let mut rx = bus.subscribe();
    let rebuild = |mem: &MemoryManager| -> HashMap<String, Value> {
        let mut map = HashMap::new();
        if let Ok(entries) = mem.read_entries() {
            for entry in &entries {
                if entry["entity_type"].as_str() == Some("project") {
                    let category = entry["after_state"]["category"].as_str().unwrap_or("");
                    if category == "work" {
                        let id = entry["entity_id"].as_str().unwrap_or("").to_string();
                        map.insert(id, entry.clone());
                    }
                }
            }
            for entry in entries {
                if entry["entity_type"].as_str() == Some("project_update") {
                    let name = entry["after_state"]["name"].as_str().unwrap_or("");
                    let new_status = entry["after_state"]["status"].as_str().unwrap_or("");
                    let category = entry["after_state"]["category"].as_str().unwrap_or("");
                    let target_id = format!("project:{}:{}", category, name.replace(' ', "_"));
                    if let Some(proj) = map.get_mut(&target_id) {
                        if let Some(obj) = proj.as_object_mut() {
                            if let Some(state) = obj.get_mut("after_state") {
                                if let Some(state_obj) = state.as_object_mut() {
                                    state_obj.insert("status".to_string(), Value::String(new_status.to_string()));
                                    state_obj.insert("last_updated".to_string(), Value::String(chrono::Utc::now().to_rfc3339()));
                                }
                            }
                        }
                    }
                }
            }
        }
        map
    };

    {
        let mut map = projects.lock().unwrap();
        *map = rebuild(&memory_manager);
    }

    loop {
        if let Ok(msg) = rx.recv().await {
            if msg.starts_with("memory:new_capture:") {
                let mut map = projects.lock().unwrap();
                *map = rebuild(&memory_manager);
            }
        }
    }
}
