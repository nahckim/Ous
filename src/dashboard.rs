use super::bus::MessageBus;
use super::memory_manager::MemoryManager;
use super::melatonin::run_melatonin;
use std::sync::Arc;
use tiny_http::{Server, Response, Header, Method};
use tokio::task;
use std::str::FromStr;
use std::collections::HashMap;
use std::sync::Mutex;
use serde_json::Value;
pub type SharedProjectMap = Arc<Mutex<HashMap<String, Value>>>;
pub async fn run_server(bus: Arc<MessageBus>, projects: SharedProjectMap, addr: &str) {
    let memory_manager = Arc::new(MemoryManager::new("data/memory/ledger.jsonl"));
    let server = Server::http(addr).unwrap();
    println!("Dashboard: http://{}", addr);
    let html = r#"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Ous Dashboard</title>
    <style>body{font-family:monospace;padding:20px;background:#0f0f0f;color:#e0e0e0;}h1,h2{color:#7eb8f7;}table{border-collapse:collapse;width:100%;}td,th{border:1px solid #333;padding:6px 10px;}tr:nth-child(even){background:#1a1a1a;}.module{display:inline-block;background:#1e1e2e;border:1px solid #444;border-radius:4px;padding:6px 12px;margin:4px;font-size:12px;}.module.memory{border-color:#7eb8f7;}.module.ai{border-color:#f7c07e;}.module.input{border-color:#7ef7a0;}.module.system{border-color:#f77e7e;}pre{background:#1a1a1a;padding:10px;border-radius:4px;overflow:auto;max-height:300px;}</style>
    </head><body>
    <h1>Ous</h1>
    <h2>System Map</h2>
    <div id="system-map">Loading...</div>
    <h2>Active Projects</h2>
    <table border="1" id="projects-table"><tr><th>Name</th><th>Status</th><th>Priority</th><th>Last Updated</th></tr></table>
    <h2>Bus Events</h2>
    <pre id="bus-events">Waiting...</pre>
    <script>
    async function fetchSystem() {
        try {
            const res = await fetch('/api/system');
            const data = await res.json();
            let html = '';
            for (const [layer, modules] of Object.entries(data)) {
                html += `<div style="margin-bottom:10px"><strong style="color:#aaa;font-size:11px;text-transform:uppercase">${layer}</strong><br>`;
                for (const m of modules) {
                    html += `<span class="module ${m.type || ''}">${m.name}${m.status ? ' — '+m.status : ''}</span>`;
                }
                html += '</div>';
            }
            document.getElementById('system-map').innerHTML = html;
        } catch(e) { document.getElementById('system-map').innerText = 'unavailable'; }
    }
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
    fetchSystem();
    fetchProjects();
    fetchStatus();
    setInterval(fetchSystem, 10000);
    setInterval(fetchProjects, 5000);
    setInterval(fetchStatus, 2000);
    </script>
    </body></html>"#;
    let content_type_html = Header::from_str("Content-Type: text/html").unwrap();
    let content_type_json = Header::from_str("Content-Type: application/json").unwrap();
    loop {
        if let Ok(mut req) = server.recv() {
            let url = req.url();
            let method = req.method();
            let resp = match (method, url) {
                (Method::Get, "/" | "/index.html") => Response::from_string(html).with_header(content_type_html.clone()),
                (Method::Get, "/status.json") => {
                    let status = std::fs::read_to_string("status.json").unwrap_or_else(|_| "{}".into());
                    Response::from_string(status).with_header(content_type_json.clone())
                }
                (Method::Get, "/api/projects") => {
                    let map = projects.lock().unwrap();
                    let list: Vec<Value> = map.values().cloned().collect();
                    Response::from_string(serde_json::to_string(&list).unwrap()).with_header(content_type_json.clone())
                }
                (Method::Get, "/api/system") => {
                    let system = std::fs::read_to_string("data/system_map.json").unwrap_or_else(|_| "{}".into());
                    Response::from_string(system).with_header(content_type_json.clone())
                }
                (Method::Post, "/capture") => {
                    let mut body = String::new();
                    if let Ok(_) = req.as_reader().read_to_string(&mut body) {
                        if let Ok(json) = serde_json::from_str::<Value>(&body) {
                            let text = json.get("text").and_then(|v| v.as_str()).unwrap_or("");
                            let source = json.get("source").and_then(|v| v.as_str()).unwrap_or("");
                            let capture_id = format!("capture:{}", uuid::Uuid::new_v4());
                            let entry = memory_manager.new_capture_entry(&capture_id, text, source);
                            match memory_manager.append(entry) {
                                Ok(_) => {
                                    println!("[Dashboard] Captured: {} from {}", text, source);
                                    Response::from_string("{\"status\":\"ok\"}").with_header(content_type_json.clone())
                                }
                                Err(e) => Response::from_string(format!("{{\"error\":\"{}\"}}", e)).with_status_code(500)
                            }
                        } else {
                            Response::from_string("{\"error\":\"invalid json\"}").with_status_code(400)
                        }
                    } else {
                        Response::from_string("{\"error\":\"read failed\"}").with_status_code(400)
                    }
                }
                (Method::Post, "/dream") => {
                    bus.publish("dream:trigger", "");
                    println!("[Dashboard] Dream trigger published");
                    Response::from_string("{\"status\":\"dream_triggered\"}").with_header(content_type_json.clone())
                }
                (Method::Post, "/trigger/dream") => {
                    bus.publish("dream:trigger", "");
                    println!("[Dashboard] Dream trigger published via /trigger/dream");
                    Response::from_string("dream triggered").with_status_code(200)
                }
                (Method::Post, "/trigger/melatonin") => {
                    task::spawn(run_melatonin(memory_manager.clone()));
                    println!("[Dashboard] Melatonin spawned via /trigger/melatonin");
                    Response::from_string("melatonin triggered").with_status_code(200)
                }
                (Method::Post, "/approve") => {
                    let mut body = String::new();
                    if let Ok(_) = req.as_reader().read_to_string(&mut body) {
                        if let Ok(json) = serde_json::from_str::<Value>(&body) {
                            if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                                std::fs::create_dir_all("data/packets/approval").ok();
                                let path = format!("data/packets/approval/{}.json", id);
                                match std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()) {
                                    Ok(_) => {
                                        println!("[Dashboard] Approved: {}", id);
                                        Response::from_string("{\"status\":\"approved\"}").with_header(content_type_json.clone())
                                    }
                                    Err(e) => Response::from_string(format!("{{\"error\":\"{}\"}}", e)).with_status_code(500)
                                }
                            } else {
                                Response::from_string("{\"error\":\"missing id\"}").with_status_code(400)
                            }
                        } else {
                            Response::from_string("{\"error\":\"invalid json\"}").with_status_code(400)
                        }
                    } else {
                        Response::from_string("{\"error\":\"read failed\"}").with_status_code(400)
                    }
                }
                _ => Response::from_string("404").with_status_code(404),
            };
            let _ = req.respond(resp);
        }
    }
}
