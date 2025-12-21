use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub model: String,
    pub created_at: String,
    pub done: bool,
    pub message: ChunkMessage,
}

#[derive(Debug, Deserialize)]
pub struct ChunkMessage {
    pub role: String,
    pub content: String,
}

pub async fn generate_response(messages: &mut Vec<Message>) -> Result<ChatChunk, reqwest::Error> {
    let client: reqwest::Client = reqwest::Client::new();

    let response = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": "mistral:7b",
            "messages": messages,
            "stream": false,
            "format": "json"
        }))
        .send()
        .await?
        .error_for_status()?;

    let res = response.json::<ChatChunk>().await?;
    Ok(res)
}
