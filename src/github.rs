use crate::auth;
use crate::issues::IssueContent;
use octocrab::Octocrab;

#[derive(Debug, Clone)]
pub struct GitHubConfig {
    token: String,
    pub owner: String,
    pub repo: String,
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
        Octocrab::builder()
            .personal_token(self.token.clone())
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
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("401") || msg.to_lowercase().contains("unauthorized") {
                    GitHubError::TokenExpired
                } else {
                    GitHubError::ApiError(msg)
                }
            })?;

        Ok(created.html_url.to_string())
    }
}
