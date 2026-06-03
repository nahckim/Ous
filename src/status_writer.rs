use crate::bus::MessageBus;
use std::sync::Arc;
use serde_json::json;
use tokio::fs;

pub async fn write_status_on_change(bus: Arc<MessageBus>) {
    let mut rx = bus.subscribe();
    let mut last_status = String::new();
    
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                let status = json!({
                    "last_event": msg,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });
                let new = status.to_string();
                if new != last_status {
                    let _ = fs::write("status.json", &new).await;
                    last_status = new;
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                let status = json!({
                    "heartbeat": chrono::Utc::now().to_rfc3339(),
                });
                let new = status.to_string();
                if new != last_status {
                    let _ = fs::write("status.json", &new).await;
                    last_status = new;
                }
            }
        }
    }
}