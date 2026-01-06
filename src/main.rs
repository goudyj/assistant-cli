use assistant::auth;
use assistant::config;
use assistant::github::GitHubConfig;
use assistant::list::IssueState;
use assistant::llm;
use assistant::tui::{self, CommandSuggestion};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();

    // Handle --logout flag
    if args.contains(&"--logout".to_string()) {
        match auth::delete_token() {
            Ok(_) => {
                println!("Logged out from GitHub.");
            }
            Err(e) => {
                eprintln!("Logout failed: {}", e);
            }
        }
        return;
    }

    // Load configuration
    let config = match config::load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Create ~/.config/assistant.json to get started.");
            return;
        }
    };

    // 1. Check for token - if none, show login screen
    let (token, config) = match auth::get_stored_token() {
        Ok(t) => (t, config),
        Err(_) => {
            // No token - show login screen
            let Some(ref client_id) = config.github_client_id else {
                eprintln!("No github_client_id in config.");
                eprintln!("Add it to ~/.config/assistant.json");
                eprintln!("Create an OAuth App at: https://github.com/settings/developers");
                return;
            };

            match tui::run_login_screen(client_id).await {
                Ok(Some(t)) => {
                    // Reload config to include the newly saved token
                    let updated_config = config::load_config().unwrap_or(config);
                    (t, updated_config)
                }
                Ok(None) => return, // User cancelled
                Err(e) => {
                    eprintln!("Login error: {}", e);
                    return;
                }
            }
        }
    };

    // 2. Determine project (from CLI arg, last_project, or prompt selection)
    let project_name_arg = args
        .iter()
        .position(|a| a == "--project")
        .and_then(|i| args.get(i + 1).cloned());

    let (project_name, project) = if let Some(name) = project_name_arg {
        // Project specified via CLI
        match config.get_project(&name) {
            Some(p) => (name, p.clone()),
            None => {
                eprintln!("Project '{}' not found.", name);
                return;
            }
        }
    } else if let Some(ref last) = config.last_project
        && let Some(p) = config.get_project(last)
    {
        // Use last project
        (last.clone(), p.clone())
    } else {
        // Show project selection
        let mut projects: Vec<_> = config.list_projects().into_iter().cloned().collect();
        projects.sort();

        if projects.is_empty() {
            eprintln!("No projects configured in ~/.config/assistant.json");
            return;
        }

        match tui::run_project_select(projects.clone()).await {
            Ok(Some(name)) => {
                let p = config.get_project(&name).unwrap().clone();
                (name, p)
            }
            Ok(None) => return, // User cancelled
            Err(e) => {
                eprintln!("Project selection error: {}", e);
                return;
            }
        }
    };

    // Save last project
    let mut config = config;
    config.set_last_project(&project_name);
    let _ = config.save();

    // 3. Build command suggestions for command palette
    let mut commands: Vec<CommandSuggestion> = vec![
        CommandSuggestion {
            name: "all".to_string(),
            description: "Show all issues (clear filters)".to_string(),
            labels: None,
        },
        CommandSuggestion {
            name: "logout".to_string(),
            description: "Logout from GitHub".to_string(),
            labels: None,
        },
        CommandSuggestion {
            name: "repository".to_string(),
            description: "Switch repository".to_string(),
            labels: None,
        },
        CommandSuggestion {
            name: "worktrees".to_string(),
            description: "Manage worktrees (view, delete, open IDE)".to_string(),
            labels: None,
        },
        CommandSuggestion {
            name: "prune".to_string(),
            description: "Clean up orphaned worktrees".to_string(),
            labels: None,
        },
        CommandSuggestion {
            name: "agent".to_string(),
            description: "Select dispatch agent (Claude Code or Opencode)".to_string(),
            labels: None,
        },
    ];

    // Add custom list commands from project config
    for (name, labels) in &project.list_commands {
        commands.push(CommandSuggestion {
            name: name.clone(),
            description: format!("Filter: {}", labels.join(", ")),
            labels: Some(labels.clone()),
        });
    }

    // 4. Build available projects list
    let available_projects: Vec<_> = config
        .projects
        .iter()
        .map(|(name, proj)| (name.clone(), proj.clone()))
        .collect();

    // 5. Launch TUI directly with issue list
    let github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.clone());
    let auto_format = config.auto_format_comments;
    let llm_endpoint = llm::default_endpoint();

    // Fetch initial issues
    match github
        .list_issues_paginated(&[], &IssueState::Open, 20, 1)
        .await
    {
        Ok((issues, has_next_page)) => {
            if let Err(e) = tui::run_issue_browser_with_pagination(
                issues,
                github,
                Some(token),
                auto_format,
                &llm_endpoint,
                Vec::new(),
                IssueState::Open,
                has_next_page,
                Some(project_name.clone()),
                project.local_path.clone(),
                project.labels.clone(),
                commands,
                available_projects,
                config.ide_command.clone(),
                config.coding_agent.clone(),
            )
            .await
            {
                eprintln!("TUI error: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch issues: {}", e);
        }
    }
}
