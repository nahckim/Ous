use super::bus::MessageBus;
use std::sync::Arc;
use tiny_http::{Server, Response, Header};
use std::str::FromStr;
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
