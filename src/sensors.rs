use super::bus::MessageBus;
use super::workspace::Bid;
use std::sync::Arc;
use sysinfo::System;
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc;

pub async fn cpu_sensor(bus: Arc<MessageBus>, bid_tx: mpsc::UnboundedSender<Bid>) {
    let mut sys = System::new_all();
    loop {
        sys.refresh_cpu();
        let cpu = sys.global_cpu_info().cpu_usage();
        bus.publish("sensor", &format!("cpu:{:.0}", cpu));
        let strength = (cpu / 100.0).clamp(0.0, 1.0);
        let bid = Bid {
            content: format!("cpu:{:.0}", cpu),
            strength,
            sensor_name: "cpu_sensor".to_string(),
        };
        let _ = bid_tx.send(bid);
        sleep(Duration::from_millis(500)).await;
    }
}
