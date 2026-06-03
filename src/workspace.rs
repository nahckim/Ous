use crate::bus::MessageBus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration};

pub struct GlobalWorkspace {
    cycle_ms: u64,
    inhibition: HashMap<String, f32>,
}

impl GlobalWorkspace {
    pub fn new(cycle_ms: u64) -> Self {
        Self {
            cycle_ms,
            inhibition: HashMap::new(),
        }
    }
    
    pub async fn run(&self, bus: Arc<MessageBus>) {
        let mut interval = tokio::time::interval(Duration::from_millis(self.cycle_ms));
        loop {
            interval.tick().await;
            // Simulated winner – replace with real bid logic later
            let winner = "cpu:42%".to_string();
            bus.publish("workspace", &winner);
        }
    }
}