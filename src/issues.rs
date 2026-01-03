use crate::llm;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueContent {
    pub type_: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

pub fn build_prompt(labels: &[String]) -> String {
    let labels_list = labels
        .iter()
        .map(|l| format!("\"{}\"", l))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"You are an assistant that writes GitHub issues in clear, concise English for a backend engineering team.

Your job:
- Take a short free-form description of work written in English or French.
- Decide if it is:
  - a **bug** (something that is broken, regressed, or does not behave as expected), or
  - a **task** (refactor, improvement, feature work, documentation, chore, etc.).
- Always answer **in English**.
- Always return output in the JSON format described below, with no extra text.

Available labels for this project: [{labels_list}]

Required output format (raw JSON, no markdown fences):

{{
  "type_": "bug" | "task",
  "title": "short English title",
  "body": "markdown formatted body",
  "labels": ["label1", "label2"]
}}

Rules for titles:
- Max about 80 characters.
- For tasks, start with a verb in the imperative (e.g. "Refactor query runner error handling", "Add retries to Kafka producer").
- For bugs, describe the observable problem (e.g. "Saving a notebook fails with 500 error", "Alerts page crashes on large tenants").

Rules for BUG issues:
- Set "type_" to "bug".
- Add "bug" to "labels" if available. Select other relevant labels from the available list.
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
- Set "type_" to "task".
- Select relevant labels from the available list.
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
- Only use labels from the available list provided above.
- If the user text is too vague, still create a useful issue: make reasonable assumptions and clearly mark uncertain parts with "TODO" items in the Acceptance criteria or notes.
- Never ask questions back to the user: always produce a complete JSON object.

Now wait for the user input and respond with a single JSON object following the schema above, with no extra commentary or explanation.
"#
    )
}

const DEFAULT_LABELS: &[&str] = &["bug", "chapter:back", "chapter:front", "chapter:sre"];

pub async fn generate_issue(
    description: &str,
) -> Result<(IssueContent, Vec<llm::Message>), Box<dyn Error>> {
    let default_labels: Vec<String> = DEFAULT_LABELS.iter().map(|s| s.to_string()).collect();
    generate_issue_with_labels(description, &default_labels, &llm::default_endpoint()).await
}

pub async fn generate_issue_with_labels(
    description: &str,
    labels: &[String],
    endpoint: &str,
) -> Result<(IssueContent, Vec<llm::Message>), Box<dyn Error>> {
    let prompt = build_prompt(labels);
    let mut messages = vec![
        llm::Message {
            role: "system".to_string(),
            content: prompt,
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

    fn test_labels() -> Vec<String> {
        vec!["bug".to_string(), "feature".to_string(), "backend".to_string()]
    }

    #[tokio::test(flavor = "current_thread")]
    async fn generate_issue_returns_parsed_content_and_history() {
        let mock = MockChatServer::new().await;
        let labels = test_labels();

        let description = "Corriger l'erreur 500 sur la page des rapports";
        let expected_messages = vec![
            llm::Message {
                role: "system".to_string(),
                content: build_prompt(&labels),
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
            labels: vec!["bug".to_string(), "backend".to_string()],
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

        let (issue, history) = generate_issue_with_labels(description, &labels, &mock.endpoint)
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
        let labels = test_labels();

        let description = "Refactor the API handler";
        let expected_messages = vec![
            llm::Message {
                role: "system".to_string(),
                content: build_prompt(&labels),
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

        let err = generate_issue_with_labels(description, &labels, &mock.endpoint)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("expected"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn build_prompt_includes_labels() {
        let labels = vec!["bug".to_string(), "enhancement".to_string()];
        let prompt = build_prompt(&labels);
        assert!(prompt.contains("\"bug\""));
        assert!(prompt.contains("\"enhancement\""));
        assert!(prompt.contains("Available labels for this project:"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn full_flow_generate_then_feedback() {
        // Step 1: Initial issue generation (first mock server)
        let mock1 = MockChatServer::new().await;
        let labels = test_labels();

        let initial_issue = IssueContent {
            type_: "bug".to_string(),
            title: "Fix login error".to_string(),
            body: "**Context**\nLogin fails.".to_string(),
            labels: vec!["bug".to_string()],
        };

        mock1
            .expect_any(serde_json::json!({
                "model": "mistral:7b",
                "created_at": "now",
                "done": true,
                "message": {
                    "role": "assistant",
                    "content": serde_json::to_string(&initial_issue).unwrap()
                }
            }))
            .await;

        let (issue, mut messages) =
            generate_issue_with_labels("fix login bug", &labels, &mock1.endpoint)
                .await
                .unwrap();

        assert_eq!(issue.title, "Fix login error");
        assert_eq!(messages.len(), 3); // system + user + assistant

        // Step 2: User provides feedback
        let feedback = "Add more details about OAuth";
        messages.push(llm::Message {
            role: "user".to_string(),
            content: feedback.to_string(),
        });

        // Step 3: LLM returns updated issue (second mock server)
        let mock2 = MockChatServer::new().await;
        let updated_issue = IssueContent {
            type_: "bug".to_string(),
            title: "Fix OAuth login error".to_string(),
            body: "**Context**\nOAuth login fails with 401 error.".to_string(),
            labels: vec!["bug".to_string(), "backend".to_string()],
        };

        mock2
            .expect_any(serde_json::json!({
                "model": "mistral:7b",
                "created_at": "now",
                "done": true,
                "message": {
                    "role": "assistant",
                    "content": serde_json::to_string(&updated_issue).unwrap()
                }
            }))
            .await;

        let response = llm::generate_response(&mut messages, &mock2.endpoint)
            .await
            .unwrap();
        let refined_issue: IssueContent =
            serde_json::from_str(&response.message.content).unwrap();

        assert_eq!(refined_issue.title, "Fix OAuth login error");
        assert!(refined_issue.body.contains("OAuth"));
        assert_eq!(refined_issue.labels.len(), 2);
    }
}
