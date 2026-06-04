use std::collections::HashMap;
use reqwest;
use serde_json::json;

#[derive(Debug, Clone, PartialEq)]
pub enum ModelType { RuleBased, TinyLocal, CloudGPT }

pub struct TaskRouter { rules: HashMap<String, ModelType> }

impl TaskRouter {
    pub fn new() -> Self {
        let mut rules = HashMap::new();
        rules.insert("cpu".to_string(), ModelType::RuleBased);
        rules.insert("keystroke".to_string(), ModelType::RuleBased);
        rules.insert("summarize".to_string(), ModelType::TinyLocal);
        rules.insert("analyze".to_string(), ModelType::CloudGPT);
        rules.insert("plan".to_string(), ModelType::CloudGPT);
        rules.insert("greet".to_string(), ModelType::TinyLocal);
        Self { rules }
    }

    pub fn route(&self, task: &str) -> ModelType {
        for (keyword, model) in &self.rules {
            if task.contains(keyword) { return model.clone(); }
        }
        ModelType::TinyLocal
    }
}

pub struct AIExecutor { router: TaskRouter }

impl AIExecutor {
    pub fn new() -> Self { Self { router: TaskRouter::new() } }

    pub async fn execute(&self, task: &str, input: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let model = self.router.route(task);
        match model {
            ModelType::RuleBased => Ok(format!("Rule result for {}: {}", task, input)),
            ModelType::TinyLocal => {
                let client = reqwest::Client::new();
                let body = json!({
                    "model": "llama3.2",
                    "prompt": input,
                    "stream": false
                });
                let res = client.post("http://localhost:11434/api/generate")
                    .json(&body)
                    .send()
                    .await?;
                let json: serde_json::Value = res.json().await?;
                let response = json["response"].as_str().unwrap_or("No response").to_string();
                Ok(response)
            },
            ModelType::CloudGPT => Ok(format!("[Cloud GPT] {}", input)),
        }
    }
}
