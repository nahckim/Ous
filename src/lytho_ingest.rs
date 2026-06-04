use tokio::fs;
use tokio::time::{interval, Duration};

pub async fn watch_lytho_folder(path: &str) {
    let mut interval = interval(Duration::from_secs(30));
    let mut last_mod = None;
    loop {
        interval.tick().await;
        if let Ok(mut entries) = fs::read_dir(path).await {
            let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let meta = match entry.metadata().await { Ok(m) => m, Err(_) => continue };
                if meta.is_file() && entry.file_name().to_string_lossy().ends_with(".csv") {
                    if let Ok(modified) = meta.modified() {
                        if last_mod.map(|lm| modified > lm).unwrap_or(true) {
                            newest = Some((entry.path(), modified));
                        }
                    }
                }
            }
            if let Some((p, t)) = newest {
                println!("[LYTHO] New export: {:?}", p);
                if let Err(e) = parse_and_update(&p).await { eprintln!("[LYTHO] Error: {}", e); }
                else { last_mod = Some(t); }
            }
        }
    }
}

async fn parse_and_update(path: &std::path::PathBuf) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut projects = Vec::new();
    for result in rdr.deserialize::<serde_json::Value>() { projects.push(result?); }
    let json_string = serde_json::to_string_pretty(&projects)?;
    tokio::fs::write("data/lytho_cache.json", json_string).await?;
    println!("[LYTHO] Updated {} projects", projects.len());
    Ok(())
}
