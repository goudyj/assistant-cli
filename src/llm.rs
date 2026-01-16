use serde::Serialize;
use std::error::Error;
use std::process::Command;

use crate::config::CodingAgentType;

#[derive(Debug, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Response structure for LLM output
#[derive(Debug)]
pub struct LlmResponse {
    pub content: String,
}

/// Generate a response using the configured coding agent CLI.
///
/// For Claude: uses `claude -p --output-format text`
/// For Opencode: uses `opencode run --format default`
pub fn generate_response(
    messages: &[Message],
    agent_type: &CodingAgentType,
) -> Result<LlmResponse, Box<dyn Error>> {
    // Build prompt from messages
    let prompt = build_prompt_from_messages(messages);

    // Execute CLI command based on agent type
    let output = match agent_type {
        CodingAgentType::Claude => execute_claude(&prompt)?,
        CodingAgentType::Opencode => execute_opencode(&prompt)?,
    };

    Ok(LlmResponse { content: output })
}

/// Build a single prompt string from the message history.
/// Combines system and user messages into a single prompt.
fn build_prompt_from_messages(messages: &[Message]) -> String {
    let mut parts = Vec::new();

    for msg in messages {
        match msg.role.as_str() {
            "system" => parts.push(msg.content.clone()),
            "user" => parts.push(format!("User: {}", msg.content)),
            "assistant" => parts.push(format!("Assistant: {}", msg.content)),
            _ => parts.push(msg.content.clone()),
        }
    }

    parts.join("\n\n")
}

/// Execute Claude CLI with the given prompt
fn execute_claude(prompt: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("claude")
        .args(["-p", "--output-format", "text", prompt])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Claude CLI failed: {}", stderr).into());
    }

    let content = String::from_utf8(output.stdout)?;
    Ok(content.trim().to_string())
}

/// Execute Opencode CLI with the given prompt
fn execute_opencode(prompt: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("opencode")
        .args(["run", prompt])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Opencode CLI failed: {}", stderr).into());
    }

    let content = String::from_utf8(output.stdout)?;
    Ok(content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_combines_messages() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are helpful.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
        ];

        let prompt = build_prompt_from_messages(&messages);
        assert!(prompt.contains("You are helpful."));
        assert!(prompt.contains("User: Hello"));
    }

    #[test]
    fn build_prompt_includes_assistant_messages() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "System prompt".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Question".to_string(),
            },
            Message {
                role: "assistant".to_string(),
                content: "Answer".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Follow up".to_string(),
            },
        ];

        let prompt = build_prompt_from_messages(&messages);
        assert!(prompt.contains("System prompt"));
        assert!(prompt.contains("User: Question"));
        assert!(prompt.contains("Assistant: Answer"));
        assert!(prompt.contains("User: Follow up"));
    }
}
