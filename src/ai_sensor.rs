use super::bus::MessageBus;
use super::ai_router::AIExecutor;
use std::sync::Arc;

pub async fn ai_worker(bus: Arc<MessageBus>) {
    let executor = AIExecutor::new();
    let mut rx = bus.subscribe();
    loop {
        if let Ok(msg) = rx.recv().await {
            if let Some(rest) = msg.strip_prefix("ai:run:") {
                if let Some(pipe_pos) = rest.find('|') {
                    let task = &rest[..pipe_pos];
                    let prompt = &rest[pipe_pos+1..];
                    match executor.execute(task, prompt).await {
                        Ok(result) => println!("[AI] Result: {}", result),
                        Err(e) => eprintln!("[AI] Error: {}", e),
                    }
                }
            }
        }
    }
}
