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
