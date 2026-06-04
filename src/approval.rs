use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};
use std::io::IsTerminal;

pub async fn request_approval(prompt: &str) -> bool {
    let pending = "data/approval/pending.txt";
    let response = "data/approval/response.txt";
    let pending_exists = std::path::Path::new(pending).exists();

    // Stdin only when explicitly in an interactive terminal with no pending file in flight.
    if std::io::stdin().is_terminal() && !pending_exists {
        print!("{} (y/n): ", prompt);
        let _ = io::stdout().flush().await;
        let mut reader = io::BufReader::new(io::stdin());
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_ok() {
            line.trim().to_lowercase() == "y"
        } else {
            false
        }
    } else {
        std::fs::create_dir_all("data/approval").ok();
        if std::fs::write(pending, prompt).is_err() {
            return false;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if std::time::Instant::now() >= deadline {
                let _ = std::fs::remove_file(pending);
                eprintln!("[Approval] Timeout waiting for file-based response");
                return false;
            }
            if let Ok(content) = std::fs::read_to_string(response) {
                let approved = content.chars().next().map(|c| c == 'y' || c == 'Y').unwrap_or(false);
                let _ = std::fs::remove_file(response);
                let _ = std::fs::remove_file(pending);
                return approved;
            }
        }
    }
}
