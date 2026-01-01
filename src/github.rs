use crate::auth;
use crate::issues::IssueContent;
use octocrab::Octocrab;

#[derive(Debug, Clone)]
pub struct GitHubConfig {
    token: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone)]
pub struct IssueSummary {
    pub number: u64,
    pub title: String,
    pub html_url: String,
    pub labels: Vec<String>,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct IssueDetail {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub labels: Vec<String>,
    pub state: String,
    pub comments: Vec<CommentInfo>,
}

#[derive(Debug, Clone)]
pub struct CommentInfo {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug)]
pub enum GitHubError {
    NotAuthenticated,
    TokenExpired,
    ApiError(String),
}

impl std::fmt::Display for GitHubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitHubError::NotAuthenticated => write!(f, "Not authenticated. Use /login first."),
            GitHubError::TokenExpired => write!(f, "Token expired or revoked. Use /logout then /login to re-authenticate."),
            GitHubError::ApiError(msg) => write!(f, "GitHub API error: {}", msg),
        }
    }
}

impl std::error::Error for GitHubError {}

impl GitHubConfig {
    pub fn new(owner: String, repo: String, token: String) -> Self {
        Self { token, owner, repo }
    }

    pub fn from_keyring(owner: String, repo: String) -> Result<Self, GitHubError> {
        let token = auth::get_stored_token().map_err(|_| GitHubError::NotAuthenticated)?;
        Ok(Self::new(owner, repo, token))
    }

    fn get_client(&self) -> Result<Octocrab, GitHubError> {
        self.get_client_with_base_url("https://api.github.com")
    }

    fn get_client_with_base_url(&self, base_url: &str) -> Result<Octocrab, GitHubError> {
        Octocrab::builder()
            .personal_token(self.token.clone())
            .base_uri(base_url)
            .map_err(|e| GitHubError::ApiError(e.to_string()))?
            .build()
            .map_err(|e| GitHubError::ApiError(e.to_string()))
    }

    pub async fn create_issue(&self, issue: &IssueContent) -> Result<String, GitHubError> {
        let client = self.get_client()?;

        let created = client
            .issues(&self.owner, &self.repo)
            .create(&issue.title)
            .body(&issue.body)
            .labels(issue.labels.clone())
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(created.html_url.to_string())
    }

    pub async fn list_issues(
        &self,
        labels: &[String],
        limit: u8,
    ) -> Result<Vec<IssueSummary>, GitHubError> {
        let client = self.get_client()?;

        let page = client
            .issues(&self.owner, &self.repo)
            .list()
            .labels(labels)
            .state(octocrab::params::State::Open)
            .per_page(limit)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(page
            .items
            .into_iter()
            .map(|issue| IssueSummary {
                number: issue.number,
                title: issue.title,
                html_url: issue.html_url.to_string(),
                labels: issue.labels.iter().map(|l| l.name.clone()).collect(),
                state: format!("{:?}", issue.state),
            })
            .collect())
    }

    pub async fn get_issue(&self, number: u64) -> Result<IssueDetail, GitHubError> {
        let client = self.get_client()?;

        let issue = client
            .issues(&self.owner, &self.repo)
            .get(number)
            .await
            .map_err(Self::map_api_error)?;

        let comments_page = client
            .issues(&self.owner, &self.repo)
            .list_comments(number)
            .per_page(50)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        let comments: Vec<CommentInfo> = comments_page
            .items
            .into_iter()
            .map(|c| CommentInfo {
                id: c.id.into_inner(),
                author: c.user.login.clone(),
                body: c.body.unwrap_or_default(),
                created_at: c.created_at.to_string(),
            })
            .collect();

        Ok(IssueDetail {
            number: issue.number,
            title: issue.title,
            body: issue.body,
            html_url: issue.html_url.to_string(),
            labels: issue.labels.iter().map(|l| l.name.clone()).collect(),
            state: format!("{:?}", issue.state),
            comments,
        })
    }

    pub async fn add_comment(
        &self,
        issue_number: u64,
        body: &str,
    ) -> Result<String, GitHubError> {
        let client = self.get_client()?;

        let comment = client
            .issues(&self.owner, &self.repo)
            .create_comment(issue_number, body)
            .await
            .map_err(Self::map_api_error)?;

        Ok(comment.html_url.to_string())
    }

    fn map_api_error(e: octocrab::Error) -> GitHubError {
        let msg = e.to_string();
        if msg.contains("401") || msg.to_lowercase().contains("unauthorized") {
            GitHubError::TokenExpired
        } else {
            GitHubError::ApiError(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_config_new() {
        let config = GitHubConfig::new(
            "owner".to_string(),
            "repo".to_string(),
            "token123".to_string(),
        );
        assert_eq!(config.owner, "owner");
        assert_eq!(config.repo, "repo");
    }

    #[test]
    fn github_error_display_not_authenticated() {
        let err = GitHubError::NotAuthenticated;
        assert!(err.to_string().contains("Not authenticated"));
    }

    #[test]
    fn github_error_display_token_expired() {
        let err = GitHubError::TokenExpired;
        assert!(err.to_string().contains("expired"));
    }

    #[test]
    fn github_error_display_api_error() {
        let err = GitHubError::ApiError("rate limit exceeded".to_string());
        assert!(err.to_string().contains("rate limit exceeded"));
    }

    #[test]
    fn issue_summary_clone() {
        let summary = IssueSummary {
            number: 42,
            title: "Test issue".to_string(),
            html_url: "https://github.com/test/test/issues/42".to_string(),
            labels: vec!["bug".to_string()],
            state: "Open".to_string(),
        };
        let cloned = summary.clone();
        assert_eq!(cloned.number, 42);
        assert_eq!(cloned.title, "Test issue");
    }

    #[test]
    fn issue_detail_with_comments() {
        let detail = IssueDetail {
            number: 1,
            title: "Bug report".to_string(),
            body: Some("Description".to_string()),
            html_url: "https://github.com/test/test/issues/1".to_string(),
            labels: vec!["bug".to_string(), "priority".to_string()],
            state: "Open".to_string(),
            comments: vec![
                CommentInfo {
                    id: 100,
                    author: "user1".to_string(),
                    body: "First comment".to_string(),
                    created_at: "2024-01-01".to_string(),
                },
            ],
        };
        assert_eq!(detail.comments.len(), 1);
        assert_eq!(detail.comments[0].author, "user1");
    }
}
