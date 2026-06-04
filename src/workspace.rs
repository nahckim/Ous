use super::bus::MessageBus;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Bid {
    pub content: String,
    pub strength: f32,
    pub sensor_name: String,
}

pub struct GlobalWorkspace {
    cycle_ms: u64,
    bid_rx: mpsc::UnboundedReceiver<Bid>,
    inhibition: HashMap<String, f32>,
}

impl GlobalWorkspace {
    pub fn new(cycle_ms: u64, bid_rx: mpsc::UnboundedReceiver<Bid>) -> Self {
        Self {
            cycle_ms,
            bid_rx,
            inhibition: HashMap::new(),
        }
    }

    pub async fn run(mut self, bus: Arc<MessageBus>) {
        let mut interval = interval(Duration::from_millis(self.cycle_ms));
        loop {
            interval.tick().await;
            let mut bids = Vec::new();
            while let Ok(bid) = self.bid_rx.try_recv() {
                bids.push(bid);
            }
            if bids.is_empty() { continue; }

            let mut best_bid: Option<Bid> = None;
            for mut bid in bids {
                if let Some(inhib) = self.inhibition.get(&bid.sensor_name) {
                    bid.strength *= inhib;
                }
                if best_bid.is_none() || bid.strength > best_bid.as_ref().unwrap().strength {
                    best_bid = Some(bid);
                }
            }
            if let Some(winner) = best_bid {
                bus.publish("workspace", &winner.content);
                // suppressed
                self.inhibition.insert(winner.sensor_name.clone(), 0.5);
                for (_, inhib) in self.inhibition.iter_mut() {
                    *inhib = (*inhib + 0.1).min(1.0);
                }
            }
        }
    }
}
