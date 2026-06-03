mod bus;
mod workspace;
mod sensors;
mod actions;
mod observer;
mod dashboard;
mod lytho_ingest;
mod project_manager;
mod status_writer;

use tokio::task;
use bus::MessageBus;
use workspace::GlobalWorkspace;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let bus = Arc::new(MessageBus::new());
    
    let ws_bus = bus.clone();
    let workspace = GlobalWorkspace::new(300);
    task::spawn(async move { workspace.run(ws_bus).await });
    
    let sensor_bus = bus.clone();
    task::spawn(async move { sensors::cpu_sensor(sensor_bus).await });
    
    let action_bus = bus.clone();
    task::spawn(async move { actions::print_action(action_bus).await });
    
    let obs_bus = bus.clone();
    task::spawn(async move { observer::pattern_observer(obs_bus).await });
    
    let dash_bus = bus.clone();
    task::spawn(async move { dashboard::run_server(dash_bus, "127.0.0.1:8080").await });
    
    let status_bus = bus.clone();
    task::spawn(async move { status_writer::write_status_on_change(status_bus).await });
    
    task::spawn(async move { lytho_ingest::watch_lytho_folder("./data/lytho").await });
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}