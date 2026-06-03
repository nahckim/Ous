use crate::bus::MessageBus;
use std::sync::Arc;
use tiny_http::{Server, Response};
use serde_json::json;

pub async fn run_server(bus: Arc<MessageBus>, addr: &str) {
    let server = Server::http(addr).unwrap();
    println!("Dashboard running at http://{}", addr);
    
    let html = r#"
<!DOCTYPE html>
<html>
<body>
<h1>Ous Dashboard</h1>
<pre id="status">Loading...</pre>
<script>
setInterval(async () => {
    const res = await fetch('/status.json');
    const data = await res.json();
    document.getElementById('status').innerText = JSON.stringify(data, null, 2);
}, 2000);
</script>
</body>
</html>
"#;
    
    loop {
        if let Ok(mut req) = server.recv() {
            let url = req.url();
            let response = if url == "/" || url == "/index.html" {
                Response::from_string(html).with_header("Content-Type", "text/html")
            } else if url == "/status.json" {
                let status = std::fs::read_to_string("status.json").unwrap_or_else(|_| "{}".into());
                Response::from_string(status).with_header("Content-Type", "application/json")
            } else {
                Response::from_string("Not found").with_status_code(404)
            };
            let _ = req.respond(response);
        }
    }
}