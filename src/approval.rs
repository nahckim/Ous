use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

pub async fn request_approval(prompt: &str) -> bool {
    print!("{} (y/n): ", prompt);
    let _ = io::stdout().flush().await;
    let mut reader = io::BufReader::new(io::stdin());
    let mut line = String::new();
    if reader.read_line(&mut line).await.is_ok() {
        line.trim().to_lowercase() == "y"
    } else {
        false
    }
}
