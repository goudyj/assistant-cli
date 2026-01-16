use crate::config::CodingAgentType;
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

pub fn generate_issue_with_labels(
    description: &str,
    labels: &[String],
    agent_type: &CodingAgentType,
) -> Result<(IssueContent, Vec<llm::Message>), Box<dyn Error>> {
    let prompt = build_prompt(labels);
    let messages = vec![
        llm::Message {
            role: "system".to_string(),
            content: prompt,
        },
        llm::Message {
            role: "user".to_string(),
            content: format!("Summarize the issue description: {}", description),
        },
    ];

    let response = llm::generate_response(&messages, agent_type)?;

    // Extract JSON from response (may contain markdown fences)
    let json_content = extract_json(&response.content)?;
    let issue_content: IssueContent = serde_json::from_str(&json_content)?;

    let mut result_messages = messages;
    result_messages.push(llm::Message {
        role: "assistant".to_string(),
        content: serde_json::json!(issue_content).to_string(),
    });

    Ok((issue_content, result_messages))
}

/// Extract JSON from a response that may contain markdown fences
fn extract_json(content: &str) -> Result<String, Box<dyn Error>> {
    let trimmed = content.trim();

    // Try to find JSON in markdown code block
    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Ok(after_fence[..end].trim().to_string());
        }
    }

    // Try to find JSON in generic code block
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Skip language identifier if present
        let content_start = after_fence.find('\n').unwrap_or(0);
        let after_lang = &after_fence[content_start..];
        if let Some(end) = after_lang.find("```") {
            return Ok(after_lang[..end].trim().to_string());
        }
    }

    // Try to find raw JSON object
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return Ok(trimmed[start..=end].to_string());
        }
    }

    // Return as-is and let serde handle parsing errors
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_includes_labels() {
        let labels = vec!["bug".to_string(), "enhancement".to_string()];
        let prompt = build_prompt(&labels);
        assert!(prompt.contains("\"bug\""));
        assert!(prompt.contains("\"enhancement\""));
        assert!(prompt.contains("Available labels for this project:"));
    }

    #[test]
    fn extract_json_from_markdown_fence() {
        let content = r#"Here is the issue:

```json
{"type_": "bug", "title": "Test", "body": "Body", "labels": []}
```

Done!"#;

        let result = extract_json(content).unwrap();
        assert!(result.contains("\"type_\": \"bug\""));
    }

    #[test]
    fn extract_json_from_generic_fence() {
        let content = r#"```
{"type_": "task", "title": "Test", "body": "Body", "labels": ["feature"]}
```"#;

        let result = extract_json(content).unwrap();
        assert!(result.contains("\"type_\": \"task\""));
    }

    #[test]
    fn extract_json_raw() {
        let content = r#"{"type_": "bug", "title": "Test", "body": "Body", "labels": []}"#;

        let result = extract_json(content).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn extract_json_with_surrounding_text() {
        let content = r#"Here is your issue:
{"type_": "bug", "title": "Test", "body": "Body", "labels": []}
Hope this helps!"#;

        let result = extract_json(content).unwrap();
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
    }
}
