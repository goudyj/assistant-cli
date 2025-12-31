use octocrab::Octocrab;

pub struct GitHubConfig {
    pub token: String,
    owner: String,
    repo: String,
}

impl GitHubConfig {
    pub fn get_github_client(&self) -> Octocrab {
        Octocrab::builder()
            .personal_token(self.token.to_string())
            .build()
            .unwrap()
    }

    pub async fn create_issue(&self) {
        let github = self.get_github_client();

        // Example usage: create an issue in a repository
        let repo_owner = "owner";
        let repo_name = "repo";
        let issue_title = "Issue Title";
        let issue_body = "Issue Body";

        let issue = github
            .issues(repo_owner, repo_name)
            .create(issue_title)
            .body(issue_body)
            .send()
            .await
            .unwrap();

        println!("Created issue: {}", issue.html_url);
    }
}
