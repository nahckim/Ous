use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;
use serde_json::json;
use chrono::Utc;

pub struct MemoryManager {
    journal_path: String,
    lock: Mutex<()>,
}

impl MemoryManager {
    pub fn new(path: &str) -> Self {
        Self {
            journal_path: path.to_string(),
            lock: Mutex::new(()),
        }
    }

    pub fn append(&self, entry: serde_json::Value) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal_path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{}", serde_json::to_string(&entry).unwrap())
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn read_entries(&self) -> Result<Vec<serde_json::Value>, String> {
        let file = std::fs::File::open(&self.journal_path).map_err(|e| e.to_string())?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line_result in reader.lines() {
            let line = line_result.map_err(|e| e.to_string())?;
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
                if entry["entity_type"].as_str() != Some("archived") {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

    pub fn rewrite_entries(&self, entries: &[serde_json::Value]) -> Result<(), String> {
        let _guard = self.lock.lock().unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.journal_path)
            .map_err(|e| e.to_string())?;
        for entry in entries {
            writeln!(file, "{}", serde_json::to_string(entry).unwrap())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn new_capture_entry(&self, capture_id: &str, content: &str, source: &str) -> serde_json::Value {
        json!({
            "entry_id": uuid::Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339(),
            "schema_version": 1,
            "entity_type": "capture",
            "entity_id": capture_id,
            "operation": "create",
            "before_state": null,
            "after_state": { "content": content, "source": source },
            "reason_capture_id": null,
            "approved_by_user": false
        })
    }

    pub fn new_project_entry(&self, name: &str, category: &str, _require_approval: bool) -> serde_json::Value {
        json!({
            "entry_id": uuid::Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339(),
            "schema_version": 1,
            "entity_type": "project",
            "entity_id": format!("project:{}:{}", category, name.replace(' ', "_")),
            "operation": "create",
            "before_state": null,
            "after_state": {
                "name": name,
                "status": "active",
                "category": category,
                "priority": "normal",
                "last_updated": Utc::now().to_rfc3339()
            },
            "reason_capture_id": null,
            "approved_by_user": false
        })
    }

    pub fn new_project_update_entry(&self, name: &str, old_status: &str, new_status: &str, category: &str) -> serde_json::Value {
        json!({
            "entry_id": uuid::Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339(),
            "schema_version": 1,
            "entity_type": "project_update",
            "entity_id": format!("project_update:{}", name.replace(' ', "_")),
            "operation": "update",
            "before_state": { "status": old_status },
            "after_state": { "name": name, "status": new_status, "category": category },
            "reason_capture_id": null,
            "approved_by_user": false
        })
    }

    pub fn new_archived_entry(&self, original_entry_id: &str) -> serde_json::Value {
        json!({
            "entry_id": uuid::Uuid::new_v4().to_string(),
            "timestamp": Utc::now().to_rfc3339(),
            "schema_version": 1,
            "entity_type": "archived",
            "original_entry_id": original_entry_id,
            "operation": "archive",
            "before_state": null,
            "after_state": null,
            "reason_capture_id": null,
            "approved_by_user": true
        })
    }
}
