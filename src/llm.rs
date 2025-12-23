use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
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

pub async fn generate_response(
    messages: &mut Vec<Message>,
    endpoint: &str,
) -> Result<ChatChunk, reqwest::Error> {
    let client: reqwest::Client = reqwest::Client::new();

    let response = client
        .post(endpoint)
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

pub fn default_endpoint() -> String {
    std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434/api/chat".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockChatServer;

    #[tokio::test(flavor = "current_thread")]
    async fn generate_response_hits_overridden_endpoint() {
        let mock = MockChatServer::new().await;

        let expected_messages = vec![Message {
            role: "user".to_string(),
            content: "ping".to_string(),
        }];
        let expected_body = serde_json::json!({
            "model": "mistral:7b",
            "messages": expected_messages,
            "stream": false,
            "format": "json"
        });

        let mock_response = serde_json::json!({
            "model": "mistral:7b",
            "created_at": "now",
            "done": true,
            "message": {
                "role": "assistant",
                "content": "pong"
            }
        });

        mock.expect_json(expected_body, mock_response).await;

        let mut messages = expected_messages.clone();
        let chunk = generate_response(&mut messages, &mock.endpoint)
            .await
            .unwrap();
        assert_eq!(chunk.message.role, "assistant");
        assert_eq!(chunk.message.content, "pong");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_response_propagates_http_error() {
        let mock = MockChatServer::new().await;

        let expected_messages = vec![Message {
            role: "user".to_string(),
            content: "ping".to_string(),
        }];
        let expected_body = serde_json::json!({
            "model": "mistral:7b",
            "messages": expected_messages,
            "stream": false,
            "format": "json"
        });

        mock.expect_status(
            expected_body,
            500,
            serde_json::json!({"error": "boom"}),
        )
        .await;

        let mut messages = expected_messages.clone();
        let err = generate_response(&mut messages, &mock.endpoint).await.unwrap_err();
        assert!(err.is_status());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_response_errors_on_invalid_json() {
        let mock = MockChatServer::new().await;

        let expected_messages = vec![Message {
            role: "user".to_string(),
            content: "ping".to_string(),
        }];
        let expected_body = serde_json::json!({
            "model": "mistral:7b",
            "messages": expected_messages,
            "stream": false,
            "format": "json"
        });

        mock.expect_status(
            expected_body,
            200,
            serde_json::json!({"message": "missing fields"}),
        )
        .await;

        let mut messages = expected_messages.clone();
        let err = generate_response(&mut messages, &mock.endpoint).await.unwrap_err();
        assert!(err.is_decode());
    }
}
