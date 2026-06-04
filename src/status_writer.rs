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
