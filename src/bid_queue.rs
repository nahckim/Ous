mod workspace {
    use super::bus::MessageBus;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::{Duration, interval};
    use std::collections::HashMap;

    // Bid submitted by a sensor
    #[derive(Debug, Clone)]
    pub struct Bid {
        pub content: String,   // e.g., "cpu:85"
        pub strength: f32,     // 0.0 - 1.0
        pub sensor_name: String,
    }

    pub struct GlobalWorkspace {
        cycle_ms: u64,
        bid_tx: mpsc::UnboundedSender<Bid>,
        bid_rx: mpsc::UnboundedReceiver<Bid>,
        inhibition: HashMap<String, f32>, // sensor_name -> remaining inhibition multiplier (0.0-1.0)
        last_winner: Option<String>,
    }

    impl GlobalWorkspace {
        pub fn new(cycle_ms: u64) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            Self {
                cycle_ms,
                bid_tx: tx,
                bid_rx: rx,
                inhibition: HashMap::new(),
                last_winner: None,
            }
        }

        // Returns a sender that sensors can clone to submit bids
        pub fn get_bid_sender(&self) -> mpsc::UnboundedSender<Bid> {
            self.bid_tx.clone()
        }

        pub async fn run(&mut self, bus: Arc<MessageBus>) {
            let mut interval = interval(Duration::from_millis(self.cycle_ms));
            loop {
                interval.tick().await;
                
                // Collect all pending bids
                let mut bids = Vec::new();
                while let Ok(bid) = self.bid_rx.try_recv() {
                    bids.push(bid);
                }

                if bids.is_empty() {
                    continue;
                }

                // Apply inhibition to bids
                let mut best_bid: Option<Bid> = None;
                for mut bid in bids {
                    if let Some(inhib) = self.inhibition.get(&bid.sensor_name) {
                        bid.strength *= *inhib;
                    }
                    if best_bid.is_none() || bid.strength > best_bid.as_ref().unwrap().strength {
                        best_bid = Some(bid);
                    }
                }

                if let Some(winner) = best_bid {
                    // Broadcast winner
                    bus.publish("workspace", &winner.content);
                    println!("[WORKSPACE] Winner: {} (strength={:.2})", winner.content, winner.strength);

                    // Update inhibition: reduce strength of winner's sensor for next cycles
                    let new_inhib = 0.5; // reduce by 50%
                    self.inhibition.insert(winner.sensor_name.clone(), new_inhib);
                    self.last_winner = Some(winner.sensor_name);

                    // Decay inhibition over time (increase toward 1.0)
                    for (_, inhib) in self.inhibition.iter_mut() {
                        *inhib = (*inhib + 0.1).min(1.0);
                    }
                }
            }
        }
    }
}