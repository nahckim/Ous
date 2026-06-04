mod bus;
mod workspace;
mod sensors;
mod actions;
mod observer;
mod dashboard;
mod status_writer;
mod lytho_ingest;
mod project_manager;
mod memory_manager;
mod ai_router;
mod ai_sensor;
mod approval;
mod packet_ingest;
mod guided_capture;
mod master_pm;
mod work_manager;
mod daily_notes;
mod minor_league;
mod dreaming;
mod melatonin;

use tokio::task;
use bus::MessageBus;
use workspace::GlobalWorkspace;
use sensors::cpu_sensor;
use actions::print_action;
use observer::pattern_observer;
use dashboard::run_server;
use dashboard::SharedProjectMap;
use status_writer::write_status_on_change;
use lytho_ingest::watch_lytho_folder;
use ai_sensor::ai_worker;
use packet_ingest::watch_packets_folder;
use guided_capture::watch_guided_folder;
use master_pm::run_master_pm;
use work_manager::run_work_manager;
use daily_notes::run_daily_writer;
use minor_league::run_pruner;
use dreaming::run_dreaming;
use melatonin::run_melatonin;
use memory_manager::MemoryManager;
use ai_router::AIExecutor;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use chrono::{Utc, Timelike, Datelike};

#[tokio::main]
async fn main() {
    let bus = Arc::new(MessageBus::new());
    let memory_manager = Arc::new(MemoryManager::new("data/memory/ledger.jsonl"));
    let ai_executor = Arc::new(AIExecutor::new());

    let (bid_tx, bid_rx) = mpsc::unbounded_channel();

    let workspace = GlobalWorkspace::new(300, bid_rx);
    let ws_bus = bus.clone();
    task::spawn(async move {
        workspace.run(ws_bus).await;
    });

    task::spawn(cpu_sensor(bus.clone(), bid_tx.clone()));

    task::spawn(print_action(bus.clone()));
    task::spawn(pattern_observer(bus.clone()));
    let project_map: SharedProjectMap = Arc::new(Mutex::new(HashMap::new()));
    task::spawn(run_server(bus.clone(), project_map.clone(), "127.0.0.1:8080"));
    task::spawn(write_status_on_change(bus.clone()));
    task::spawn(watch_lytho_folder("./data/lytho"));
    task::spawn(ai_worker(bus.clone()));

    let pbus = bus.clone();
    let pmem = memory_manager.clone();
    let pai = ai_executor.clone();
    task::spawn(async move { watch_packets_folder(pbus, pmem, pai, "./data/packets").await });

    let gbus = bus.clone();
    let gmem = memory_manager.clone();
    let gai = ai_executor.clone();
    task::spawn(async move { watch_guided_folder(gbus, gmem, gai, "./data/guided").await });

    task::spawn(run_master_pm(bus.clone(), memory_manager.clone()));
    task::spawn(run_work_manager(bus.clone(), memory_manager.clone(), project_map));
    task::spawn(run_daily_writer(memory_manager.clone()));
    task::spawn(run_pruner(memory_manager.clone()));
    task::spawn(run_dreaming(bus.clone(), memory_manager.clone(), ai_executor.clone()));

    let sched_bus = bus.clone();
    let sched_mem = memory_manager.clone();
    task::spawn(async move {
        let mut last_melatonin_day = -1;
        let mut last_dream_day = -1;
        loop {
            let now = Utc::now();
            let hour = now.hour();
            let day = now.ordinal();

            if hour == 1 && last_melatonin_day != day as i32 {
                println!("[Scheduler] Triggering melatonin at 1AM UTC");
                task::spawn(run_melatonin(sched_mem.clone()));
                last_melatonin_day = day as i32;
            }

            if hour == 2 && last_dream_day != day as i32 {
                println!("[Scheduler] Publishing dream:trigger at 2AM UTC");
                sched_bus.publish("dream:trigger", "");
                last_dream_day = day as i32;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
