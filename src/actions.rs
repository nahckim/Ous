use crate::bus::MessageBus;
use std::sync::Arc;

pub async fn print_action(bus: Arc<MessageBus>) {
    let mut rx = bus.subscribe();
    loop {
        match rx.recv().await {
            Ok(msg) if msg.starts_with("workspace:") => {
                let parts: Vec<&str> = msg.splitn(2, ':').collect();
                if parts.len() == 2 {
                    println!("[CONSCIOUS] {}", parts[1]);
                }
            }
            _ => {}
        }
    }
}