// src/main.rs â€“ Ous final working version (all fixes applied)

mod bus {
    use tokio::sync::broadcast;
    pub struct MessageBus { sender: broadcast::Sender<String> }
    impl MessageBus {
        pub fn new() -> Self { let (tx, _) = broadcast::channel(256); Self { sender: tx } }
        pub fn publish(&self, topic: &str, payload: &str) {
            let _ = self.sender.send(format!("{}:{}", topic, payload));
        }
        pub fn subscribe(&self) -> broadcast::Receiver<String> {
            self.sender.subscribe()
        }
    }
}

mod workspace {
    use super::bus::MessageBus;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, interval};
    use std::collections::HashMap;

    #[derive(Debug, Clone)]
    pub struct Bid {
        pub content: String,
        pub strength: f32,
        pub sensor_name: String,
    }

    pub struct GlobalWorkspace {
        cycle_ms: u64,
        bid_rx: mpsc::UnboundedReceiver<Bid>,
        inhibition: HashMap<String, f32>,
    }

    impl GlobalWorkspace {
        pub fn new(cycle_ms: u64, bid_rx: mpsc::UnboundedReceiver<Bid>) -> Self {
            Self {
                cycle_ms,
                bid_rx,
                inhibition: HashMap::new(),
            }
        }

        pub async fn run(mut self, bus: Arc<MessageBus>) {
            let mut interval = interval(Duration::from_millis(self.cycle_ms));
            loop {
                interval.tick().await;
                let mut bids = Vec::new();
                while let Ok(bid) = self.bid_rx.try_recv() {
                    bids.push(bid);
                }
                if bids.is_empty() { continue; }

                let mut best_bid: Option<Bid> = None;
                for mut bid in bids {
                    if let Some(inhib) = self.inhibition.get(&bid.sensor_name) {
                        bid.strength *= inhib;
                    }
                    if best_bid.is_none() || bid.strength > best_bid.as_ref().unwrap().strength {
                        best_bid = Some(bid);
                    }
                }
                if let Some(winner) = best_bid {
                    bus.publish("workspace", &winner.content);
                    println!("[WORKSPACE] Winner: {} (strength={:.2})", winner.content, winner.strength);
                    self.inhibition.insert(winner.sensor_name.clone(), 0.5);
                    for (_, inhib) in self.inhibition.iter_mut() {
                        *inhib = (*inhib + 0.1).min(1.0);
                    }
                }
            }
        }
    }
}

mod sensors {
    use super::bus::MessageBus;
    use super::workspace::Bid;
    use std::sync::Arc;
    use sysinfo::System;
    use tokio::time::{sleep, Duration};
    use tokio::sync::mpsc;

    pub async fn cpu_sensor(bus: Arc<MessageBus>, bid_tx: mpsc::UnboundedSender<Bid>) {
        let mut sys = System::new_all();
        loop {
            sys.refresh_cpu();
            let cpu = sys.global_cpu_info().cpu_usage();
            bus.publish("sensor", &format!("cpu:{:.0}", cpu));
            let strength = (cpu / 100.0).clamp(0.0, 1.0);
            let bid = Bid {
                content: format!("cpu:{:.0}", cpu),
                strength,
                sensor_name: "cpu_sensor".to_string(),
            };
            let _ = bid_tx.send(bid);
            sleep(Duration::from_millis(500)).await;
        }
    }
}

mod actions {
    use super::bus::MessageBus;
    use std::sync::Arc;
    pub async fn print_action(bus: Arc<MessageBus>) {
        let mut rx = bus.subscribe();
        loop {
            if let Ok(msg) = rx.recv().await {
                if msg.starts_with("workspace:") {
                    println!("[CONSCIOUS] {}", &msg[10..]);
                }
            }
        }
    }
}

mod observer {
    use super::bus::MessageBus;
    use std::collections::VecDeque;
    use std::sync::Arc;
    pub async fn pattern_observer(bus: Arc<MessageBus>) {
        let mut rx = bus.subscribe();
        let mut history = VecDeque::with_capacity(100);
        loop {
            if let Ok(msg) = rx.recv().await {
                if msg.starts_with("pattern:") || msg.starts_with("workspace:") { continue; }
                history.push_back(msg.clone());
                if history.len() > 100 { history.pop_front(); }
                let last10: Vec<&String> = history.iter().rev().take(10).collect();
                let count = last10.iter().filter(|&m| *m == &msg).count();
                if count >= 3 {
                    println!("[OBSERVER] Pattern detected: {}", msg);
                    bus.publish("pattern", &format!("pattern:{}", msg));
                }
            }
        }
    }
}

mod dashboard {
    use super::bus::MessageBus;
    use std::sync::Arc;
    use tiny_http::{Server, Response, Header};
    use std::str::FromStr;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use serde_json::Value;

    pub type SharedProjectMap = Arc<Mutex<HashMap<String, Value>>>;

    pub async fn run_server(_bus: Arc<MessageBus>, projects: SharedProjectMap, addr: &str) {
        let server = Server::http(addr).unwrap();
        println!("Dashboard: http://{}", addr);
        let html = r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Ous Dashboard</title></head><body>
        <h1>Ous Dashboard</h1>
        <h2>Active Work Projects</h2>
        <table border="1" id="projects-table"><tr><th>Name</th><th>Status</th><th>Priority</th><th>Last Updated</th></tr></tr>
        <h2>Live Bus Events</h2>
        <pre id="bus-events">Waiting for events...</pre>
        <script>
        async function fetchProjects() {
            try {
                const res = await fetch('/api/projects');
                const proj = await res.json();
                let rows = '<tr><th>Name</th><th>Status</th><th>Priority</th><th>Last Updated</th></tr>';
                for (const p of proj) {
                    rows += `<tr><td>${p.name}</td><td>${p.status}</td><td>${p.priority}</td><td>${p.last_updated}</td></tr>`;
                }
                document.getElementById('projects-table').innerHTML = rows;
            } catch(e) { console.error(e); }
        }
        async function fetchStatus() {
            try {
                const res = await fetch('/status.json');
                const data = await res.json();
                document.getElementById('bus-events').innerText = JSON.stringify(data, null, 2);
            } catch(e) { console.error(e); }
        }
        fetchProjects();
        setInterval(fetchProjects, 5000);
        fetchStatus();
        setInterval(fetchStatus, 2000);
        </script>
        </body></html>"#;
        let content_type_html = Header::from_str("Content-Type: text/html").unwrap();
        let content_type_json = Header::from_str("Content-Type: application/json").unwrap();

        loop {
            if let Ok(req) = server.recv() {
                let url = req.url();
                let resp = match url {
                    "/" | "/index.html" => Response::from_string(html).with_header(content_type_html.clone()),
                    "/status.json" => {
                        let status = std::fs::read_to_string("status.json").unwrap_or_else(|_| "{}".into());
                        Response::from_string(status).with_header(content_type_json.clone())
                    }
                    "/api/projects" => {
                        let map = projects.lock().unwrap();
                        let list: Vec<Value> = map.values().cloned().collect();
                        Response::from_string(serde_json::to_string(&list).unwrap()).with_header(content_type_json.clone())
                    }
                    _ => Response::from_string("404").with_status_code(404),
                };
                let _ = req.respond(resp);
            }
        }
    }
}

mod status_writer {
    use super::bus::MessageBus;
    use std::sync::Arc;
    use serde_json::json;
    use tokio::fs;
    pub async fn write_status_on_change(bus: Arc<MessageBus>) {
        let mut rx = bus.subscribe();
        let mut last = String::new();
        loop {
            tokio::select! {
                Ok(msg) = rx.recv() => {
                    let s = json!({ "last_event": msg, "timestamp": chrono::Utc::now().to_rfc3339() }).to_string();
                    if s != last { let _ = fs::write("status.json", &s).await; last = s; }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                    let s = json!({ "heartbeat": chrono::Utc::now().to_rfc3339() }).to_string();
                    if s != last { let _ = fs::write("status.json", &s).await; last = s; }
                }
            }
        }
    }
}

mod lytho_ingest {
    use tokio::fs;
    use tokio::time::{interval, Duration};
    use std::path::PathBuf;
    pub async fn watch_lytho_folder(path: &str) {
        let mut interval = interval(Duration::from_secs(30));
        let mut last_mod = None;
        loop {
            interval.tick().await;
            if let Ok(mut entries) = fs::read_dir(path).await {
                let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let meta = match entry.metadata().await { Ok(m) => m, Err(_) => continue };
                    if meta.is_file() && entry.file_name().to_string_lossy().ends_with(".csv") {
                        if let Ok(modified) = meta.modified() {
                            if last_mod.map(|lm| modified > lm).unwrap_or(true) {
                                newest = Some((entry.path(), modified));
                            }
                        }
                    }
                }
                if let Some((p, t)) = newest {
                    println!("[LYTHO] New export: {:?}", p);
                    if let Err(e) = parse_and_update(&p).await { eprintln!("[LYTHO] Error: {}", e); }
                    else { last_mod = Some(t); }
                }
            }
        }
    }
    async fn parse_and_update(path: &PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut rdr = csv::Reader::from_reader(content.as_bytes());
        let mut projects = Vec::new();
        for result in rdr.deserialize::<serde_json::Value>() { projects.push(result?); }
        let json_string = serde_json::to_string_pretty(&projects)?;
        tokio::fs::write("data/lytho_cache.json", json_string).await?;
        println!("[LYTHO] Updated {} projects", projects.len());
        Ok(())
    }
}

mod project_manager {
    use serde_json::Value;
    use std::fs;
    pub fn load_projects() -> Vec<Value> {
        fs::read_to_string("data/lytho_cache.json")
            .map(|s| serde_json::from_str(&s).unwrap_or_default())
            .unwrap_or_default()
    }
}

mod memory_manager {
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
            for line in reader.lines() {
                let line = line.map_err(|e| e.to_string())?;
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
}

mod ai_router {
    use std::collections::HashMap;
    use reqwest;
    use serde_json::json;
    #[derive(Debug, Clone, PartialEq)]
    pub enum ModelType { RuleBased, TinyLocal, CloudGPT }
    pub struct TaskRouter { rules: HashMap<String, ModelType> }
    impl TaskRouter {
        pub fn new() -> Self {
            let mut rules = HashMap::new();
            rules.insert("cpu".to_string(), ModelType::RuleBased);
            rules.insert("keystroke".to_string(), ModelType::RuleBased);
            rules.insert("summarize".to_string(), ModelType::TinyLocal);
            rules.insert("analyze".to_string(), ModelType::CloudGPT);
            rules.insert("plan".to_string(), ModelType::CloudGPT);
            rules.insert("greet".to_string(), ModelType::TinyLocal);
            Self { rules }
        }
        pub fn route(&self, task: &str) -> ModelType {
            for (keyword, model) in &self.rules {
                if task.contains(keyword) { return model.clone(); }
            }
            ModelType::TinyLocal
        }
    }
    pub struct AIExecutor { router: TaskRouter }
    impl AIExecutor {
        pub fn new() -> Self { Self { router: TaskRouter::new() } }
        pub async fn execute(&self, task: &str, input: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let model = self.router.route(task);
            match model {
                ModelType::RuleBased => Ok(format!("Rule result for {}: {}", task, input)),
                ModelType::TinyLocal => {
                    let client = reqwest::Client::new();
                    let body = json!({
                        "model": "llama3.2",
                        "prompt": input,
                        "stream": false
                    });
                    let res = client.post("http://localhost:11435/api/generate")
                        .json(&body)
                        .send()
                        .await?;
                    let json: serde_json::Value = res.json().await?;
                    let response = json["response"].as_str().unwrap_or("No response").to_string();
                    Ok(response)
                },
                ModelType::CloudGPT => Ok(format!("[Cloud GPT] {}", input)),
            }
        }
    }
}

mod ai_sensor {
    use super::bus::MessageBus;
    use super::ai_router::AIExecutor;
    use std::sync::Arc;
    pub async fn ai_worker(bus: Arc<MessageBus>) {
        let executor = AIExecutor::new();
        let mut rx = bus.subscribe();
        loop {
            if let Ok(msg) = rx.recv().await {
                if let Some(rest) = msg.strip_prefix("ai:run:") {
                    if let Some(pipe_pos) = rest.find('|') {
                        let task = &rest[..pipe_pos];
                        let prompt = &rest[pipe_pos+1..];
                        match executor.execute(task, prompt).await {
                            Ok(result) => println!("[AI] Result: {}", result),
                            Err(e) => eprintln!("[AI] Error: {}", e),
                        }
                    }
                }
            }
        }
    }
}

mod approval {
    use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
    pub async fn request_approval(prompt: &str) -> bool {
        print!("{} (y/n): ", prompt);
        let _ = io::stdout().flush().await;
        let mut reader = io::BufReader::new(io::stdin());
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_ok() {
            line.trim().to_lowercase() == "y"
        } else {
            false
        }
    }
}

mod packet_ingest {
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
    enum Packet {
        Regular { id: String, task: String, prompt: String, require_approval: Option<bool> },
        Project { name: String, context_hint: Option<String>, require_approval: Option<bool> },
        ProjectUpdate { name: String, status: String, require_approval: Option<bool> },
        Dream { require_approval: Option<bool> },
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
}

mod guided_capture {
    use super::bus::MessageBus;
    use super::memory_manager::MemoryManager;
    use super::ai_router::AIExecutor;
    use super::approval;
    use std::sync::Arc;
    use serde::Deserialize;
    use tokio::fs;
    use tokio::time::{interval, Duration};
    use std::path::PathBuf;
    use serde_json::json;
    use chrono::Utc;
    #[derive(Debug, Deserialize)]
    struct GuidedPacket {
        id: String,
        prompt: String,
        context_hint: Option<String>,
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
                                            "Output ONLY valid JSON (no extra text, no markdown). Use this format: {{\"entity_type\":\"...\", \"entity_id\":\"...\", \"after_state\":{{\"summary\":\"...\"}}}}. User note: {}",
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
                                        processed.insert(file_name);
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
}

mod master_pm {
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
}

mod work_manager {
    use super::bus::MessageBus;
    use super::memory_manager::MemoryManager;
    use std::sync::Arc;
    use std::collections::HashMap;
    use serde_json::Value;
    use std::sync::Mutex;
    use super::dashboard::SharedProjectMap;

    pub async fn run_work_manager(bus: Arc<MessageBus>, memory_manager: Arc<MemoryManager>, projects: SharedProjectMap) {
        let mut rx = bus.subscribe();
        let rebuild = |mem: &MemoryManager| -> HashMap<String, Value> {
            let mut map = HashMap::new();
            if let Ok(entries) = mem.read_entries() {
                // First pass: add base projects
                for entry in &entries {
                    if entry["entity_type"].as_str() == Some("project") {
                        let category = entry["after_state"]["category"].as_str().unwrap_or("");
                        if category == "work" {
                            let id = entry["entity_id"].as_str().unwrap_or("").to_string();
                            map.insert(id, entry.clone());
                        }
                    }
                }
                // Second pass: apply updates
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

        // Initial load
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
}

mod daily_notes {
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
}

mod minor_league {
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
}

mod dreaming {
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
}

use tokio::task;
use bus::MessageBus;
use workspace::GlobalWorkspace;
use sensors::cpu_sensor;
use actions::print_action;
use observer::pattern_observer;
use dashboard::run_server;
use dashboard::SharedProjectMap;
use status_writer::write_status_on_change;
use lytho_ingest::watch_lytho_folder;
use ai_sensor::ai_worker;
use packet_ingest::watch_packets_folder;
use guided_capture::watch_guided_folder;
use master_pm::run_master_pm;
use work_manager::run_work_manager;
use daily_notes::run_daily_writer;
use minor_league::run_pruner;
use dreaming::run_dreaming;
use memory_manager::MemoryManager;
use ai_router::AIExecutor;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let bus = Arc::new(MessageBus::new());
    let memory_manager = Arc::new(MemoryManager::new("data/memory/ledger.jsonl"));
    let ai_executor = Arc::new(AIExecutor::new());

    let (bid_tx, bid_rx) = mpsc::unbounded_channel();

    let workspace = GlobalWorkspace::new(300, bid_rx);
    let ws_bus = bus.clone();
    task::spawn(async move {
        workspace.run(ws_bus).await;
    });

    task::spawn(cpu_sensor(bus.clone(), bid_tx.clone()));

    task::spawn(print_action(bus.clone()));
    task::spawn(pattern_observer(bus.clone()));
    let project_map: SharedProjectMap = Arc::new(Mutex::new(HashMap::new()));
    task::spawn(run_server(bus.clone(), project_map.clone(), "127.0.0.1:8080"));
    task::spawn(write_status_on_change(bus.clone()));
    task::spawn(watch_lytho_folder("./data/lytho"));
    task::spawn(ai_worker(bus.clone()));

    let pbus = bus.clone();
    let pmem = memory_manager.clone();
    let pai = ai_executor.clone();
    task::spawn(async move { watch_packets_folder(pbus, pmem, pai, "./data/packets").await });

    let gbus = bus.clone();
    let gmem = memory_manager.clone();
    let gai = ai_executor.clone();
    task::spawn(async move { watch_guided_folder(gbus, gmem, gai, "./data/guided").await });

    task::spawn(run_master_pm(bus.clone(), memory_manager.clone()));
    task::spawn(run_work_manager(bus.clone(), memory_manager.clone(), project_map));
    task::spawn(run_daily_writer(memory_manager.clone()));
    task::spawn(run_pruner(memory_manager.clone()));
    task::spawn(run_dreaming(bus.clone(), memory_manager.clone(), ai_executor.clone()));

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

