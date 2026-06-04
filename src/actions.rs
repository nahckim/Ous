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
