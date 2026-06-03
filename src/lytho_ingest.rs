use tokio::fs;
use tokio::time::{interval, Duration};
use std::path::PathBuf;

pub async fn watch_lytho_folder(path: &str) {
    let mut interval = interval(Duration::from_secs(30));
    let mut last_modified = None;
    
    loop {
        interval.tick().await;
        if let Ok(mut entries) = fs::read_dir(path).await {
            let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let meta = if let Ok(m) = entry.metadata().await { m } else { continue };
                if meta.is_file() && entry.file_name().to_string_lossy().ends_with(".csv") {
                    if let Ok(modified) = meta.modified() {
                        if last_modified.map(|lm| modified > lm).unwrap_or(true) {
                            newest = Some((entry.path(), modified));
                        }
                    }
                }
            }
            if let Some((path, time)) = newest {
                println!("[LYTHO] New export found: {:?}", path);
                if let Err(e) = parse_and_update(&path).await {
                    eprintln!("[LYTHO] Parse error: {}", e);
                } else {
                    last_modified = Some(time);
                }
            }
        }
    }
}

async fn parse_and_update(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut projects = Vec::new();
    for result in rdr.deserialize() {
        let record: serde_json::Value = result?;
        projects.push(record);
    }
    let _ = fs::write("data/lytho_cache.json", serde_json::to_string_pretty(&projects)?).await;
    println!("[LYTHO] Updated {} projects", projects.len());
    Ok(())
}