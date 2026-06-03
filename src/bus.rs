use tokio::sync::broadcast;

pub struct MessageBus {
    sender: broadcast::Sender<String>,
}

impl MessageBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { sender: tx }
    }
    
    pub fn publish(&self, topic: &str, payload: &str) {
        let msg = format!("{}:{}", topic, payload);
        let _ = self.sender.send(msg);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}