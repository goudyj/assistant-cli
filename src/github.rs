use crate::auth;
use crate::issues::IssueContent;
use crate::list::IssueState;
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
    pub assignees: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IssueDetail {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub labels: Vec<String>,
    pub state: String,
    pub assignees: Vec<String>,
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

    /// Create an issue and return (html_url, IssueSummary)
    pub async fn create_issue(&self, issue: &IssueContent) -> Result<(String, IssueSummary), GitHubError> {
        let client = self.get_client()?;

        let created = client
            .issues(&self.owner, &self.repo)
            .create(&issue.title)
            .body(&issue.body)
            .labels(issue.labels.clone())
            .send()
            .await
            .map_err(Self::map_api_error)?;

        let summary = IssueSummary {
            number: created.number,
            title: created.title,
            html_url: created.html_url.to_string(),
            labels: created.labels.iter().map(|l| l.name.clone()).collect(),
            state: "Open".to_string(),
            assignees: created.assignees.iter().map(|a| a.login.clone()).collect(),
        };

        Ok((created.html_url.to_string(), summary))
    }

    pub async fn list_issues(
        &self,
        labels: &[String],
        state: &IssueState,
        limit: u8,
    ) -> Result<Vec<IssueSummary>, GitHubError> {
        let (issues, _) = self.list_issues_paginated(labels, state, limit, 1).await?;
        Ok(issues)
    }

    /// List issues with pagination support
    /// Returns (issues, has_next_page)
    pub async fn list_issues_paginated(
        &self,
        labels: &[String],
        state: &IssueState,
        per_page: u8,
        page_num: u32,
    ) -> Result<(Vec<IssueSummary>, bool), GitHubError> {
        let client = self.get_client()?;

        let octocrab_state = match state {
            IssueState::Open => octocrab::params::State::Open,
            IssueState::Closed => octocrab::params::State::Closed,
            IssueState::All => octocrab::params::State::All,
        };

        let page = client
            .issues(&self.owner, &self.repo)
            .list()
            .labels(labels)
            .state(octocrab_state)
            .per_page(per_page)
            .page(page_num)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        // Check if there's a next page by looking at the page info
        let has_next = page.next.is_some();

        let issues = page
            .items
            .into_iter()
            .map(|issue| IssueSummary {
                number: issue.number,
                title: issue.title,
                html_url: issue.html_url.to_string(),
                labels: issue.labels.iter().map(|l| l.name.clone()).collect(),
                state: format!("{:?}", issue.state),
                assignees: issue.assignees.iter().map(|u| u.login.clone()).collect(),
            })
            .collect();

        Ok((issues, has_next))
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
            assignees: issue.assignees.iter().map(|u| u.login.clone()).collect(),
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

    pub async fn close_issue(&self, issue_number: u64) -> Result<(), GitHubError> {
        let client = self.get_client()?;

        client
            .issues(&self.owner, &self.repo)
            .update(issue_number)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(())
    }

    pub async fn reopen_issue(&self, issue_number: u64) -> Result<(), GitHubError> {
        let client = self.get_client()?;

        client
            .issues(&self.owner, &self.repo)
            .update(issue_number)
            .state(octocrab::models::IssueState::Open)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(())
    }

    /// List available assignees for the repository
    pub async fn list_assignees(&self) -> Result<Vec<String>, GitHubError> {
        let client = self.get_client()?;

        let page = client
            .repos(&self.owner, &self.repo)
            .list_collaborators()
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(page.items.into_iter().map(|u| u.author.login).collect())
    }

    /// Assign users to an issue
    pub async fn assign_issue(
        &self,
        issue_number: u64,
        assignees: &[String],
    ) -> Result<(), GitHubError> {
        let client = self.get_client()?;

        client
            .issues(&self.owner, &self.repo)
            .update(issue_number)
            .assignees(assignees)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(())
    }

    /// Remove assignees from an issue
    pub async fn unassign_issue(
        &self,
        issue_number: u64,
        assignees: &[String],
    ) -> Result<(), GitHubError> {
        let client = self.get_client()?;

        // To unassign, we need to set the assignees to the current list minus the ones to remove
        // First, get current assignees
        let issue = client
            .issues(&self.owner, &self.repo)
            .get(issue_number)
            .await
            .map_err(Self::map_api_error)?;

        let current: Vec<String> = issue.assignees.iter().map(|u| u.login.clone()).collect();
        let new_assignees: Vec<String> = current
            .into_iter()
            .filter(|a| !assignees.contains(a))
            .collect();

        client
            .issues(&self.owner, &self.repo)
            .update(issue_number)
            .assignees(&new_assignees)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        Ok(())
    }

    /// Search issues in the repository using GitHub Search API
    pub async fn search_issues(&self, query: &str) -> Result<Vec<IssueSummary>, GitHubError> {
        let client = self.get_client()?;

        // Build search query: repo:owner/repo is:issue <user_query>
        let search_query = format!("repo:{}/{} is:issue {}", self.owner, self.repo, query);

        let page = client
            .search()
            .issues_and_pull_requests(&search_query)
            .per_page(50)
            .send()
            .await
            .map_err(Self::map_api_error)?;

        let issues = page
            .items
            .into_iter()
            .map(|issue| IssueSummary {
                number: issue.number,
                title: issue.title,
                html_url: issue.html_url.to_string(),
                labels: issue.labels.iter().map(|l| l.name.clone()).collect(),
                state: format!("{:?}", issue.state),
                assignees: issue.assignees.iter().map(|u| u.login.clone()).collect(),
            })
            .collect();

        Ok(issues)
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
            assignees: vec!["user1".to_string()],
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
            assignees: vec!["user1".to_string()],
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

    fn mock_issue_response(number: u64, state: &str) -> serde_json::Value {
        serde_json::json!({
            "id": number,
            "node_id": "I_test",
            "number": number,
            "title": "Test issue",
            "state": state,
            "state_reason": null,
            "locked": false,
            "html_url": format!("https://github.com/owner/repo/issues/{}", number),
            "url": format!("https://api.github.com/repos/owner/repo/issues/{}", number),
            "repository_url": "https://api.github.com/repos/owner/repo",
            "labels_url": "https://api.github.com/repos/owner/repo/issues/{}/labels{{/name}}",
            "comments_url": "https://api.github.com/repos/owner/repo/issues/{}/comments",
            "events_url": "https://api.github.com/repos/owner/repo/issues/{}/events",
            "labels": [],
            "user": {
                "login": "test",
                "id": 1,
                "node_id": "U_test",
                "avatar_url": "https://avatars.githubusercontent.com/u/1",
                "gravatar_id": "",
                "url": "https://api.github.com/users/test",
                "html_url": "https://github.com/test",
                "followers_url": "https://api.github.com/users/test/followers",
                "following_url": "https://api.github.com/users/test/following{/other_user}",
                "gists_url": "https://api.github.com/users/test/gists{/gist_id}",
                "starred_url": "https://api.github.com/users/test/starred{/owner}{/repo}",
                "subscriptions_url": "https://api.github.com/users/test/subscriptions",
                "organizations_url": "https://api.github.com/users/test/orgs",
                "repos_url": "https://api.github.com/users/test/repos",
                "events_url": "https://api.github.com/users/test/events{/privacy}",
                "received_events_url": "https://api.github.com/users/test/received_events",
                "type": "User",
                "site_admin": false
            },
            "assignees": [],
            "milestone": null,
            "comments": 0,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "closed_at": null,
            "author_association": "OWNER",
            "body": "Test body"
        })
    }

    #[tokio::test(flavor = "current_thread")]
    async fn close_issue_success() {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_issue_response(123, "closed")))
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let result = client
            .issues("owner", "repo")
            .update(123u64)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reopen_issue_success() {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/456"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_issue_response(456, "open")))
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let result = client
            .issues("owner", "repo")
            .update(456u64)
            .state(octocrab::models::IssueState::Open)
            .send()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn close_issue_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/999"))
            .respond_with(
                ResponseTemplate::new(404).set_body_json(serde_json::json!({
                    "message": "Not Found"
                })),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let result = client
            .issues("owner", "repo")
            .update(999u64)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await;

        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn close_issue_forbidden() {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/123"))
            .respond_with(
                ResponseTemplate::new(403).set_body_json(serde_json::json!({
                    "message": "Forbidden"
                })),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let result = client
            .issues("owner", "repo")
            .update(123u64)
            .state(octocrab::models::IssueState::Closed)
            .send()
            .await;

        assert!(result.is_err());
    }

    fn mock_issue_with_assignees(number: u64, assignees: Vec<&str>) -> serde_json::Value {
        let assignee_objects: Vec<serde_json::Value> = assignees
            .iter()
            .map(|login| {
                serde_json::json!({
                    "login": login,
                    "id": 1,
                    "node_id": "U_test",
                    "avatar_url": "https://avatars.githubusercontent.com/u/1",
                    "gravatar_id": "",
                    "url": format!("https://api.github.com/users/{}", login),
                    "html_url": format!("https://github.com/{}", login),
                    "followers_url": format!("https://api.github.com/users/{}/followers", login),
                    "following_url": format!("https://api.github.com/users/{}/following{{/other_user}}", login),
                    "gists_url": format!("https://api.github.com/users/{}/gists{{/gist_id}}", login),
                    "starred_url": format!("https://api.github.com/users/{}/starred{{/owner}}{{/repo}}", login),
                    "subscriptions_url": format!("https://api.github.com/users/{}/subscriptions", login),
                    "organizations_url": format!("https://api.github.com/users/{}/orgs", login),
                    "repos_url": format!("https://api.github.com/users/{}/repos", login),
                    "events_url": format!("https://api.github.com/users/{}/events{{/privacy}}", login),
                    "received_events_url": format!("https://api.github.com/users/{}/received_events", login),
                    "type": "User",
                    "site_admin": false
                })
            })
            .collect();

        serde_json::json!({
            "id": number,
            "node_id": "I_test",
            "number": number,
            "title": "Test issue",
            "state": "open",
            "state_reason": null,
            "locked": false,
            "html_url": format!("https://github.com/owner/repo/issues/{}", number),
            "url": format!("https://api.github.com/repos/owner/repo/issues/{}", number),
            "repository_url": "https://api.github.com/repos/owner/repo",
            "labels_url": "https://api.github.com/repos/owner/repo/issues/{}/labels{/name}",
            "comments_url": "https://api.github.com/repos/owner/repo/issues/{}/comments",
            "events_url": "https://api.github.com/repos/owner/repo/issues/{}/events",
            "labels": [],
            "user": {
                "login": "test",
                "id": 1,
                "node_id": "U_test",
                "avatar_url": "https://avatars.githubusercontent.com/u/1",
                "gravatar_id": "",
                "url": "https://api.github.com/users/test",
                "html_url": "https://github.com/test",
                "followers_url": "https://api.github.com/users/test/followers",
                "following_url": "https://api.github.com/users/test/following{/other_user}",
                "gists_url": "https://api.github.com/users/test/gists{/gist_id}",
                "starred_url": "https://api.github.com/users/test/starred{/owner}{/repo}",
                "subscriptions_url": "https://api.github.com/users/test/subscriptions",
                "organizations_url": "https://api.github.com/users/test/orgs",
                "repos_url": "https://api.github.com/users/test/repos",
                "events_url": "https://api.github.com/users/test/events{/privacy}",
                "received_events_url": "https://api.github.com/users/test/received_events",
                "type": "User",
                "site_admin": false
            },
            "assignees": assignee_objects,
            "milestone": null,
            "comments": 0,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "closed_at": null,
            "author_association": "OWNER",
            "body": "Test body"
        })
    }

    #[tokio::test(flavor = "current_thread")]
    async fn assign_issue_success() {
        let server = MockServer::start().await;

        // Mock the PATCH request to add assignees
        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_issue_with_assignees(123, vec!["user1"])),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let assignees = vec!["user1".to_string()];
        let result = client
            .issues("owner", "repo")
            .update(123u64)
            .assignees(&assignees)
            .send()
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unassign_issue_removes_assignee() {
        let server = MockServer::start().await;

        // Mock GET request to get current issue with assignees
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/issues/123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_issue_with_assignees(123, vec!["user1", "user2"])),
            )
            .mount(&server)
            .await;

        // Mock PATCH request to update assignees (remove user1)
        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/123"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_issue_with_assignees(123, vec!["user2"])),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        // Get the current issue
        let issue = client.issues("owner", "repo").get(123u64).await;
        assert!(issue.is_ok());
        let issue = issue.unwrap();
        assert_eq!(issue.assignees.len(), 2);

        // Update with user2 only (removing user1)
        let assignees = vec!["user2".to_string()];
        let result = client
            .issues("owner", "repo")
            .update(123u64)
            .assignees(&assignees)
            .send()
            .await;

        assert!(result.is_ok());
    }

    fn mock_issues_list_response(issues: Vec<(u64, &str)>) -> Vec<serde_json::Value> {
        issues
            .into_iter()
            .map(|(number, title)| {
                serde_json::json!({
                    "id": number,
                    "node_id": format!("I_{}", number),
                    "number": number,
                    "title": title,
                    "state": "open",
                    "state_reason": null,
                    "locked": false,
                    "html_url": format!("https://github.com/owner/repo/issues/{}", number),
                    "url": format!("https://api.github.com/repos/owner/repo/issues/{}", number),
                    "repository_url": "https://api.github.com/repos/owner/repo",
                    "labels_url": "https://api.github.com/repos/owner/repo/issues/{}/labels{/name}",
                    "comments_url": "https://api.github.com/repos/owner/repo/issues/{}/comments",
                    "events_url": "https://api.github.com/repos/owner/repo/issues/{}/events",
                    "labels": [],
                    "user": {
                        "login": "test",
                        "id": 1,
                        "node_id": "U_test",
                        "avatar_url": "https://avatars.githubusercontent.com/u/1",
                        "gravatar_id": "",
                        "url": "https://api.github.com/users/test",
                        "html_url": "https://github.com/test",
                        "followers_url": "https://api.github.com/users/test/followers",
                        "following_url": "https://api.github.com/users/test/following{/other_user}",
                        "gists_url": "https://api.github.com/users/test/gists{/gist_id}",
                        "starred_url": "https://api.github.com/users/test/starred{/owner}{/repo}",
                        "subscriptions_url": "https://api.github.com/users/test/subscriptions",
                        "organizations_url": "https://api.github.com/users/test/orgs",
                        "repos_url": "https://api.github.com/users/test/repos",
                        "events_url": "https://api.github.com/users/test/events{/privacy}",
                        "received_events_url": "https://api.github.com/users/test/received_events",
                        "type": "User",
                        "site_admin": false
                    },
                    "assignees": [],
                    "milestone": null,
                    "comments": 0,
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z",
                    "closed_at": null,
                    "author_association": "OWNER",
                    "body": "Test body"
                })
            })
            .collect()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_issues_paginated_first_page() {
        let server = MockServer::start().await;

        let issues = mock_issues_list_response(vec![
            (1, "First issue"),
            (2, "Second issue"),
        ]);

        // Mock the first page with a Link header indicating next page
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/issues"))
            .and(query_param("page", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&issues)
                    .insert_header(
                        "Link",
                        "<https://api.github.com/repos/owner/repo/issues?page=2>; rel=\"next\", <https://api.github.com/repos/owner/repo/issues?page=3>; rel=\"last\"",
                    ),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let page = client
            .issues("owner", "repo")
            .list()
            .per_page(2)
            .page(1u32)
            .send()
            .await;

        assert!(page.is_ok());
        let page = page.unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(page.next.is_some()); // has_next_page = true
    }

    #[tokio::test(flavor = "current_thread")]
    async fn list_issues_paginated_last_page() {
        let server = MockServer::start().await;

        let issues = mock_issues_list_response(vec![(3, "Third issue")]);

        // Mock the last page without a Link header for next
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/issues"))
            .and(query_param("page", "2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&issues)
                    .insert_header(
                        "Link",
                        "<https://api.github.com/repos/owner/repo/issues?page=1>; rel=\"prev\", <https://api.github.com/repos/owner/repo/issues?page=1>; rel=\"first\"",
                    ),
            )
            .mount(&server)
            .await;

        let client = Octocrab::builder()
            .personal_token("token".to_string())
            .base_uri(&server.uri())
            .unwrap()
            .build()
            .unwrap();

        let page = client
            .issues("owner", "repo")
            .list()
            .per_page(2)
            .page(2u32)
            .send()
            .await;

        assert!(page.is_ok());
        let page = page.unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.next.is_none()); // has_next_page = false
    }

    #[test]
    fn issue_summary_no_duplicates() {
        // Test that issues can be deduplicated by number
        let issues = vec![
            IssueSummary {
                number: 1,
                title: "First".to_string(),
                html_url: "https://github.com/test/test/issues/1".to_string(),
                labels: vec![],
                state: "Open".to_string(),
                assignees: vec![],
            },
            IssueSummary {
                number: 2,
                title: "Second".to_string(),
                html_url: "https://github.com/test/test/issues/2".to_string(),
                labels: vec![],
                state: "Open".to_string(),
                assignees: vec![],
            },
            IssueSummary {
                number: 1, // Duplicate
                title: "First (duplicate)".to_string(),
                html_url: "https://github.com/test/test/issues/1".to_string(),
                labels: vec![],
                state: "Open".to_string(),
                assignees: vec![],
            },
        ];

        // Deduplicate by number (keep first occurrence)
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<_> = issues
            .into_iter()
            .filter(|i| seen.insert(i.number))
            .collect();

        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].number, 1);
        assert_eq!(unique[1].number, 2);
    }
}
