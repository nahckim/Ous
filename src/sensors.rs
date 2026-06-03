use crate::bus::MessageBus;
use std::sync::Arc;
use sysinfo::System;
use tokio::time::{sleep, Duration};

pub async fn cpu_sensor(bus: Arc<MessageBus>) {
    let mut sys = System::new_all();
    loop {
        sys.refresh_cpu();
        let cpu_percent = sys.global_cpu_info().cpu_usage();
        let msg = format!("cpu:{:.0}", cpu_percent);
        bus.publish("sensor", &msg);
        sleep(Duration::from_millis(500)).await;
    }
}