/// Options for the /list command
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListOptions {
    pub labels: Vec<String>,
    pub state: IssueState,
    pub search: Option<String>,
}

/// State filter for issues
#[derive(Debug, Clone, Default, PartialEq)]
pub enum IssueState {
    #[default]
    Open,
    Closed,
    All,
}

impl ListOptions {
    /// Parse /list command arguments
    ///
    /// Syntax:
    /// - `/list` - no arguments, all open issues
    /// - `/list bug` - filter by label "bug" (if known label)
    /// - `/list bug,feature` - filter by multiple labels
    /// - `/list auth` - search for "auth" (if not a known label)
    /// - `/list bug auth` - label "bug" + search "auth"
    /// - `/list "error handling"` - search with spaces
    /// - `/list --state=closed` - closed issues
    /// - `/list --state=all bug` - all issues with label "bug"
    pub fn parse(args: &str, known_labels: &[String]) -> Self {
        let mut options = ListOptions::default();
        let args = args.trim();

        if args.is_empty() {
            return options;
        }

        let tokens = tokenize(args);

        for token in tokens {
            if token.starts_with("--state=") {
                let state_value = token.strip_prefix("--state=").unwrap();
                options.state = match state_value.to_lowercase().as_str() {
                    "closed" => IssueState::Closed,
                    "all" => IssueState::All,
                    _ => IssueState::Open,
                };
            } else if token.contains(',') {
                // Multiple labels separated by comma
                for label in token.split(',') {
                    let label = label.trim();
                    if !label.is_empty() {
                        options.labels.push(label.to_string());
                    }
                }
            } else if is_known_label(&token, known_labels) {
                options.labels.push(token);
            } else {
                // Treat as search query
                if options.search.is_none() {
                    options.search = Some(token);
                } else {
                    // Append to existing search
                    let existing = options.search.take().unwrap();
                    options.search = Some(format!("{} {}", existing, token));
                }
            }
        }

        options
    }
}

/// Tokenize the argument string, respecting quoted strings
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in input.chars() {
        match c {
            '"' => {
                if in_quotes {
                    // End of quoted string
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    in_quotes = false;
                } else {
                    // Start of quoted string
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    in_quotes = true;
                }
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Check if a token matches a known label (case-insensitive)
fn is_known_label(token: &str, known_labels: &[String]) -> bool {
    let lower_token = token.to_lowercase();
    known_labels
        .iter()
        .any(|label| label.to_lowercase() == lower_token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known_labels() -> Vec<String> {
        vec![
            "bug".to_string(),
            "feature".to_string(),
            "Bug".to_string(),
            "customer".to_string(),
        ]
    }

    #[test]
    fn parse_empty_args() {
        let options = ListOptions::parse("", &known_labels());
        assert_eq!(options.labels, Vec::<String>::new());
        assert_eq!(options.state, IssueState::Open);
        assert_eq!(options.search, None);
    }

    #[test]
    fn parse_single_label() {
        let options = ListOptions::parse("bug", &known_labels());
        assert_eq!(options.labels, vec!["bug"]);
        assert_eq!(options.state, IssueState::Open);
        assert_eq!(options.search, None);
    }

    #[test]
    fn parse_multi_labels_comma() {
        let options = ListOptions::parse("bug,feature", &known_labels());
        assert_eq!(options.labels, vec!["bug", "feature"]);
        assert_eq!(options.search, None);
    }

    #[test]
    fn parse_search_keyword() {
        let options = ListOptions::parse("auth", &known_labels());
        assert!(options.labels.is_empty());
        assert_eq!(options.search, Some("auth".to_string()));
    }

    #[test]
    fn parse_label_and_search() {
        let options = ListOptions::parse("bug auth", &known_labels());
        assert_eq!(options.labels, vec!["bug"]);
        assert_eq!(options.search, Some("auth".to_string()));
    }

    #[test]
    fn parse_quoted_search() {
        let options = ListOptions::parse("\"error handling\"", &known_labels());
        assert!(options.labels.is_empty());
        assert_eq!(options.search, Some("error handling".to_string()));
    }

    #[test]
    fn parse_state_closed() {
        let options = ListOptions::parse("--state=closed", &known_labels());
        assert_eq!(options.state, IssueState::Closed);
    }

    #[test]
    fn parse_state_all() {
        let options = ListOptions::parse("--state=all", &known_labels());
        assert_eq!(options.state, IssueState::All);
    }

    #[test]
    fn parse_state_closed_with_label() {
        let options = ListOptions::parse("--state=closed bug", &known_labels());
        assert_eq!(options.state, IssueState::Closed);
        assert_eq!(options.labels, vec!["bug"]);
    }

    #[test]
    fn parse_complex() {
        let options = ListOptions::parse("bug --state=all \"error handling\"", &known_labels());
        assert_eq!(options.state, IssueState::All);
        assert_eq!(options.labels, vec!["bug"]);
        assert_eq!(options.search, Some("error handling".to_string()));
    }

    #[test]
    fn parse_case_insensitive_label() {
        let options = ListOptions::parse("Bug", &known_labels());
        assert_eq!(options.labels, vec!["Bug"]);
    }

    #[test]
    fn tokenize_simple() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn tokenize_quoted() {
        let tokens = tokenize("hello \"world of\" test");
        assert_eq!(tokens, vec!["hello", "world of", "test"]);
    }

    #[test]
    fn tokenize_empty_quotes() {
        let tokens = tokenize("hello \"\" world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }
}
