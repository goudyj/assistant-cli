use crate::llm;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub struct IssueContent {
    pub type_: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

pub static PROMPT_ISSUE: &str = r#"You are an assistant that writes GitHub issues in clear, concise English for a backend engineering team.

Your job:
- Take a short free-form description of work written in English or French.
- Decide if it is:
  - a **bug** (something that is broken, regressed, or does not behave as expected), or
  - a **task** (refactor, improvement, feature work, documentation, chore, etc.).
- Always answer **in English**.
- Always return output in the JSON format described below, with no extra text.

Required output format (raw JSON, no markdown fences):

{
  "type_": "bug" | "task",
  "title": "short English title",
  "body": "markdown formatted body",
  "labels": ["bug", "chapter:back"]
}

Rules for titles:
- Max about 80 characters.
- For tasks, start with a verb in the imperative (e.g. "Refactor query runner error handling", "Add retries to Kafka producer").
- For bugs, describe the observable problem (e.g. "Saving a notebook fails with 500 error", "Alerts page crashes on large tenants").

Rules for BUG issues:
- Set "type" to "bug".
- Add "bug" to "labels". You can add the other relevent labels, "chapter:back", "chapter:front", "chapter:sre".
- In "body", always include AT LEAST these sections in markdown:

  **Context**
  - Briefly explain what the feature/workflow is and where the bug appears.

  **Steps to reproduce**
  1. Step 1…
  2. Step 2…
  3. Step 3…

- If there is not enough information to provide real steps, keep the section but write:
  "Not enough information to provide detailed steps. TODO: clarify with reporter."
- When relevant, also add these sections after the two above:

  **Expected behavior**
  - One or two sentences about what should happen.

  **Actual behavior**
  - One or two sentences about what actually happens.

  **Additional information**
  - Logs, error messages, environment, feature flags, etc., only if they are clearly implied by the input.

Rules for TASK issues:
- Set "type" to "task".
- Use the following labels if relevent: "chapter:back", "chapter:front", "chapter:sre"
- In "body", use this structure in markdown:

  **Context**
  - Why this task is needed, what problem or goal it relates to.

  **Goal**
  - One or two sentences describing the desired end state of the work.

  **Acceptance criteria**
  - Bullet list of clear, testable criteria.
  - Use checkboxes and start each line with "- [ ]".
  - If important information is missing, add a bullet like:
    - [ ] Clarify XYZ with product/tech lead.

  **Technical notes**
  - Optional. Implementation hints, impacted modules, APIs, risks, edge cases.

Language and translation rules:
- If the input is in French, translate it to English while preserving technical terms, code samples, identifiers, and logs.
- Keep all class names, function names, variable names, file paths, and SOL/SQL queries exactly as given.
- Be concise and write for experienced engineers (no fluff, no over-explaining).

Important:
- If the user text is too vague, still create a useful issue: make reasonable assumptions and clearly mark uncertain parts with "TODO" items in the Acceptance criteria or notes.
- Never ask questions back to the user: always produce a complete JSON object.

Now wait for the user input and respond with a single JSON object following the schema above, with no extra commentary or explanation.
"#;

pub async fn generate_issue(
    description: &str,
) -> Result<(IssueContent, Vec<llm::Message>), Box<dyn Error>> {
    generate_issue_with_endpoint(description, &llm::default_endpoint()).await
}

pub async fn generate_issue_with_endpoint(
    description: &str,
    endpoint: &str,
) -> Result<(IssueContent, Vec<llm::Message>), Box<dyn Error>> {
    let mut messages = vec![
        llm::Message {
            role: "system".to_string(),
            content: PROMPT_ISSUE.to_string(),
        },
        llm::Message {
            role: "user".to_string(),
            content: format!("Summarize the issue description: {}", description),
        },
    ];

    let content: String = llm::generate_response(&mut messages, endpoint)
        .await?
        .message
        .content;
    let issue_content: IssueContent = serde_json::from_str(&content)?;
    messages.push(llm::Message {
        role: "assistant".to_string(),
        content: serde_json::json!(issue_content).to_string(),
    });
    Ok((issue_content, messages))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockChatServer;

    #[tokio::test(flavor = "current_thread")]
    async fn generate_issue_returns_parsed_content_and_history() {
        let mock = MockChatServer::new().await;

        let description = "Corriger l'erreur 500 sur la page des rapports";
        let expected_messages = vec![
            llm::Message {
                role: "system".to_string(),
                content: PROMPT_ISSUE.to_string(),
            },
            llm::Message {
                role: "user".to_string(),
                content: format!("Summarize the issue description: {}", description),
            },
        ];
        let expected_body = serde_json::json!({
            "model": "mistral:7b",
            "messages": expected_messages,
            "stream": false,
            "format": "json"
        });

        let mocked_issue = IssueContent {
            type_: "bug".to_string(),
            title: "Fix report page 500 error".to_string(),
            body: "**Context**\n- The reports page fails.\n\n**Steps to reproduce**\n1. Open reports\n2. Observe 500".to_string(),
            labels: vec!["bug".to_string(), "chapter:back".to_string()],
        };

        let response_body = serde_json::json!({
            "model": "mistral:7b",
            "created_at": "now",
            "done": true,
            "message": {
                "role": "assistant",
                "content": serde_json::to_string(&mocked_issue).unwrap()
            }
        });

        mock.expect_json(expected_body, response_body).await;

        let (issue, history) = generate_issue_with_endpoint(description, &mock.endpoint)
            .await
            .unwrap();
        assert_eq!(issue.title, mocked_issue.title);
        assert_eq!(issue.type_, "bug");
        assert_eq!(issue.labels, mocked_issue.labels);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, "system");
        assert_eq!(history[2].role, "assistant");
        assert_eq!(history[2].content, serde_json::json!(issue).to_string());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_issue_fails_on_invalid_json_response() {
        let mock = MockChatServer::new().await;

        let description = "Refactor the API handler";
        let expected_messages = vec![
            llm::Message {
                role: "system".to_string(),
                content: PROMPT_ISSUE.to_string(),
            },
            llm::Message {
                role: "user".to_string(),
                content: format!("Summarize the issue description: {}", description),
            },
        ];
        let expected_body = serde_json::json!({
            "model": "mistral:7b",
            "messages": expected_messages,
            "stream": false,
            "format": "json"
        });

        mock.expect_status(
            expected_body,
            200,
            serde_json::json!({
                "model": "mistral:7b",
                "created_at": "now",
                "done": true,
                "message": {
                    "role": "assistant",
                    "content": "not-json"
                }
            }),
        )
        .await;

        let err = generate_issue_with_endpoint(description, &mock.endpoint)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("expected"),
            "unexpected error: {err}"
        );
    }
}
