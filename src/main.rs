use assistant::agents::{
    attach_tmux_command, is_tmux_session_running, kill_agent, list_tmux_sessions,
    tmux_session_name, SessionManager,
};
use assistant::auth::{self, DeviceFlowAuth};
use assistant::config::{self, Config, ProjectConfig};
use assistant::github::GitHubConfig;
use assistant::issues::{self, IssueContent};
use assistant::list::{IssueState, ListOptions};
use assistant::llm;
use assistant::tui;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use dialoguer::{theme::ColorfulTheme, Select};
use reedline::{Completer, DefaultPrompt, Reedline, Signal, Span, Suggestion};
use std::io;
use std::sync::{Arc, RwLock};

struct AppState {
    config: Option<Config>,
    current_project: Option<ProjectConfig>,
    current_project_name: Option<String>,
    issue_session: Option<IssueSession>,
    cached_token: Option<String>,
}

struct IssueSession {
    issue: IssueContent,
    messages: Vec<llm::Message>,
}

/// Static commands that are always available
const STATIC_COMMANDS: &[&str] = &[
    "/login",
    "/logout",
    "/repository",
    "/repo",
    "/list",
    "/issue",
    "/ok",
    "/agents",
    "/sessions",
    "/attach",
    "/help",
    "/quit",
    "/exit",
];

/// Completer that provides both static and dynamic commands
struct AssistantCompleter {
    dynamic_commands: Arc<RwLock<Vec<String>>>,
}

impl AssistantCompleter {
    fn new() -> Self {
        Self {
            dynamic_commands: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn get_dynamic_commands(&self) -> Vec<String> {
        self.dynamic_commands
            .read()
            .map(|cmds| cmds.clone())
            .unwrap_or_default()
    }
}

impl Completer for AssistantCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        if !line.starts_with('/') {
            return vec![];
        }

        let prefix = &line[..pos];
        let mut suggestions = Vec::new();

        for cmd in STATIC_COMMANDS {
            if cmd.starts_with(prefix) {
                suggestions.push(Suggestion {
                    value: cmd.to_string(),
                    description: None,
                    style: None,
                    extra: None,
                    span: Span::new(0, pos),
                    append_whitespace: true,
                    match_indices: None,
                });
            }
        }

        for cmd in self.get_dynamic_commands() {
            if cmd.starts_with(prefix) {
                suggestions.push(Suggestion {
                    value: cmd.clone(),
                    description: Some("list issues".to_string()),
                    style: None,
                    extra: None,
                    span: Span::new(0, pos),
                    append_whitespace: false,
                    match_indices: None,
                });
            }
        }

        suggestions
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Load token once at startup to avoid repeated keyring prompts
    let cached_token = auth::get_stored_token().ok();

    let mut state = AppState {
        config: None,
        current_project: None,
        current_project_name: None,
        issue_session: None,
        cached_token,
    };

    match config::load_config() {
        Ok(cfg) => {
            let projects: Vec<_> = cfg.list_projects();
            if !projects.is_empty() {
                print_colored_message(
                    &format!(
                        "Config loaded. Projects: {}\n",
                        projects
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    Color::Green,
                );
            }

            if let Some(ref last_project) = cfg.last_project
                && let Some(project) = cfg.get_project(last_project) {
                    print_colored_message(
                        &format!("Restored project: {}/{}\n", project.owner, project.repo),
                        Color::Green,
                    );
                    state.current_project = Some(project.clone());
                    state.current_project_name = Some(last_project.clone());
                }

            state.config = Some(cfg);
        }
        Err(e) => {
            print_colored_message(
                &format!(
                    "Warning: {}\nCreate ~/.config/assistant.json to get started.\n",
                    e
                ),
                Color::DarkYellow,
            );
        }
    }

    if state.cached_token.is_some() {
        print_colored_message("GitHub: logged in\n", Color::Green);
    } else {
        print_colored_message("GitHub: not logged in. Use /login to authenticate.\n", Color::DarkYellow);
    }

    print_colored_message(
        "Commands: /login, /repository <name>, /issue <desc>, /ok, /quit\n",
        Color::DarkMagenta,
    );

    // Create completer with shared dynamic commands
    let completer = AssistantCompleter::new();
    let dynamic_commands_ref = Arc::clone(&completer.dynamic_commands);

    // Initialize dynamic commands from current project if any
    if let Some(ref project) = state.current_project {
        let cmds: Vec<String> = project.list_command_names().into_iter().cloned().collect();
        if let Ok(mut dc) = dynamic_commands_ref.write() {
            *dc = cmds.into_iter().map(|c| format!("/{}", c)).collect();
        }
    }

    let mut rl = Reedline::create().with_completer(Box::new(completer));
    let prompt = DefaultPrompt::default();

    loop {
        let line = match rl.read_line(&prompt) {
            Ok(Signal::Success(input)) => input.trim().to_string(),
            Ok(Signal::CtrlD) | Ok(Signal::CtrlC) | Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        if line.starts_with('/') {
            let mut parts = line.splitn(2, ' ');
            let command = parts.next().unwrap_or("");
            let rest = parts.next().unwrap_or("").trim();

            match command {
                "/quit" | "/exit" => break,

                "/login" => {
                    handle_login_command(&mut state).await;
                }

                "/logout" => {
                    handle_logout_command(&mut state);
                }

                "/repository" | "/repo" => {
                    handle_repository_command(rest, &mut state, &dynamic_commands_ref);
                }

                "/list" => {
                    handle_list_with_options(rest, &state).await;
                }

                "/issue" => {
                    if rest.is_empty() {
                        print_colored_message("Usage: /issue <description>\n", Color::DarkMagenta);
                        continue;
                    }
                    handle_issue_command(rest, &mut state).await;
                }

                "/ok" => {
                    handle_ok_command(&mut state).await;
                }

                "/help" => {
                    print_help(&state);
                }

                "/agents" => {
                    handle_agents_command(rest, &state).await;
                }

                "/sessions" => {
                    handle_sessions_command(&state);
                }

                "/attach" => {
                    handle_attach_command(rest, &state);
                }

                _ => {
                    // Check if it's a dynamic list command
                    let cmd_name = command.trim_start_matches('/');
                    if let Some(ref project) = state.current_project
                        && let Some(labels) = project.get_list_command_labels(cmd_name) {
                            handle_list_command(labels.clone(), &state).await;
                            continue;
                        }
                    print_colored_message("Unknown command. Type /help.\n", Color::DarkMagenta);
                }
            }
            continue;
        }

        if let Some(session) = state.issue_session.as_mut() {
            if let Err(err) = handle_feedback(&line, session).await {
                eprintln!("Error updating issue: {err:#?}");
                state.issue_session = None;
            }
        } else {
            print_colored_message("Type /help for commands.\n", Color::DarkMagenta);
        }
    }
}

async fn handle_login_command(state: &mut AppState) {
    if state.cached_token.is_some() {
        print_colored_message("Already logged in. Use /logout first to re-authenticate.\n", Color::DarkYellow);
        return;
    }

    let client_id = match &state.config {
        Some(cfg) => cfg.github_client_id.clone(),
        None => None,
    };

    let Some(client_id) = client_id else {
        print_colored_message(
            "No github_client_id in config. Add it to ~/.config/assistant.json\n\
             Example: { \"github_client_id\": \"Ov23li...\", \"projects\": {...} }\n\
             Create an OAuth App at: https://github.com/settings/developers\n",
            Color::Red,
        );
        return;
    };

    print_colored_message("Starting GitHub authentication...\n", Color::DarkMagenta);

    let auth_flow = match DeviceFlowAuth::start(&client_id).await {
        Ok(flow) => flow,
        Err(e) => {
            print_colored_message(&format!("Failed to start auth: {}\n", e), Color::Red);
            return;
        }
    };

    print_colored_message(
        &format!(
            "\nOpen this URL in your browser:\n  {}\n\nAnd enter the code: {}\n\nWaiting for authorization...\n",
            auth_flow.verification_uri, auth_flow.user_code
        ),
        Color::Cyan,
    );

    if let Err(e) = auth_flow.open_browser() {
        print_colored_message(&format!("Could not open browser: {}\n", e), Color::DarkYellow);
    }

    match auth_flow.poll_for_token().await {
        Ok(token) => {
            if let Err(e) = auth::store_token(&token) {
                print_colored_message(&format!("Failed to store token: {}\n", e), Color::Red);
                return;
            }
            state.cached_token = Some(token);
            print_colored_message("Successfully logged in to GitHub!\n", Color::Green);
        }
        Err(e) => {
            print_colored_message(&format!("Authentication failed: {}\n", e), Color::Red);
        }
    }
}

fn handle_logout_command(state: &mut AppState) {
    match auth::delete_token() {
        Ok(_) => {
            state.cached_token = None;
            print_colored_message("Logged out from GitHub.\n", Color::Green);
        }
        Err(e) => print_colored_message(&format!("Logout failed: {}\n", e), Color::Red),
    }
}

fn handle_repository_command(
    name: &str,
    state: &mut AppState,
    dynamic_commands: &Arc<RwLock<Vec<String>>>,
) {
    let Some(ref mut cfg) = state.config else {
        print_colored_message(
            "No config loaded. Create ~/.config/assistant.json\n",
            Color::DarkYellow,
        );
        return;
    };

    let selected_name = if name.is_empty() {
        let mut projects: Vec<_> = cfg.list_projects().into_iter().cloned().collect();
        projects.sort();

        if projects.is_empty() {
            print_colored_message("No projects configured.\n", Color::DarkYellow);
            return;
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select a project")
            .items(&projects)
            .default(0)
            .interact_opt();

        match selection {
            Ok(Some(index)) => projects[index].clone(),
            Ok(None) => {
                print_colored_message("Selection cancelled.\n", Color::DarkMagenta);
                return;
            }
            Err(_) => {
                print_colored_message("Selection failed.\n", Color::Red);
                return;
            }
        }
    } else {
        name.to_string()
    };

    match cfg.get_project(&selected_name) {
        Some(project) => {
            print_colored_message(
                &format!(
                    "Selected project: {}/{}\nLabels: {}\n",
                    project.owner,
                    project.repo,
                    project.labels.join(", ")
                ),
                Color::Green,
            );

            // Update dynamic commands for autocomplete
            let cmds: Vec<String> = project.list_command_names().into_iter().cloned().collect();
            if let Ok(mut dc) = dynamic_commands.write() {
                *dc = cmds.iter().map(|c| format!("/{}", c)).collect();
            }
            if !cmds.is_empty() {
                print_colored_message(
                    &format!("List commands: {}\n", cmds.iter().map(|c| format!("/{}", c)).collect::<Vec<_>>().join(", ")),
                    Color::Green,
                );
            }

            state.current_project = Some(project.clone());
            state.current_project_name = Some(selected_name.clone());
            state.issue_session = None;

            cfg.set_last_project(&selected_name);
            if let Err(e) = cfg.save() {
                print_colored_message(&format!("Warning: could not save config: {}\n", e), Color::DarkYellow);
            }
        }
        None => {
            let projects: Vec<_> = cfg.list_projects();
            print_colored_message(
                &format!(
                    "Project '{}' not found. Available: {}\n",
                    selected_name,
                    projects
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Color::DarkYellow,
            );
        }
    }
}

async fn handle_issue_command(description: &str, state: &mut AppState) {
    let labels = match &state.current_project {
        Some(project) => project.labels.clone(),
        None => {
            print_colored_message(
                "No project selected. Use /repository <name> first.\n",
                Color::DarkYellow,
            );
            return;
        }
    };

    match issues::generate_issue_with_labels(description, &labels, &llm::default_endpoint()).await {
        Ok((issue, messages)) => {
            state.issue_session = Some(IssueSession { issue, messages });
            if let Some(ref session) = state.issue_session {
                print_issue(&session.issue);
                print_colored_message(
                    "Give feedback to adapt the issue or type /ok to create it.\n",
                    Color::DarkMagenta,
                );
            }
        }
        Err(err) => eprintln!("Error generating issue: {err:#?}"),
    }
}

async fn handle_list_with_options(args: &str, state: &AppState) {
    let Some(ref project) = state.current_project else {
        print_colored_message("No project selected. Use /repository <name> first.\n", Color::DarkYellow);
        return;
    };

    let Some(ref token) = state.cached_token else {
        print_colored_message("Not authenticated. Use /login first.\n", Color::Red);
        return;
    };

    // Parse options using known labels from project config
    let options = ListOptions::parse(args, &project.labels);

    let github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.clone());
    let github_token = state.cached_token.clone();

    let state_str = match options.state {
        IssueState::Open => "open",
        IssueState::Closed => "closed",
        IssueState::All => "all",
    };

    if options.labels.is_empty() && options.search.is_none() {
        print_colored_message(&format!("Fetching {} issues...\n", state_str), Color::DarkMagenta);
    } else {
        let filter_desc = if !options.labels.is_empty() {
            format!("labels: {}", options.labels.join(", "))
        } else {
            String::new()
        };
        let search_desc = options.search.as_ref().map(|s| format!("search: '{}'", s)).unwrap_or_default();
        let desc = [filter_desc, search_desc].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
        print_colored_message(&format!("Fetching {} issues ({})...\n", state_str, desc), Color::DarkMagenta);
    }

    match github.list_issues_paginated(&options.labels, &options.state, 20, 1).await {
        Ok((mut issues, has_next_page)) => {
            // Apply local search filter if specified
            if let Some(ref query) = options.search {
                issues.retain(|issue| matches_query(issue, query));
            }

            if issues.is_empty() {
                print_colored_message("No issues found matching criteria.\n", Color::DarkYellow);
                return;
            }

            let auto_format = state
                .config
                .as_ref()
                .map(|c| c.auto_format_comments)
                .unwrap_or(false);

            if let Err(e) =
                tui::run_issue_browser_with_pagination(
                    issues,
                    github,
                    github_token,
                    auto_format,
                    &llm::default_endpoint(),
                    options.labels.clone(),
                    options.state.clone(),
                    has_next_page,
                    state.current_project_name.clone(),
                    project.local_path.clone(),
                ).await
            {
                print_colored_message(&format!("TUI error: {}\n", e), Color::Red);
            }
        }
        Err(e) => {
            print_colored_message(&format!("Failed to fetch issues: {}\n", e), Color::Red);
        }
    }
}

/// Check if an issue matches a search query (case-insensitive)
fn matches_query(issue: &assistant::github::IssueSummary, query: &str) -> bool {
    let query_lower = query.to_lowercase();

    // Search in title
    if issue.title.to_lowercase().contains(&query_lower) {
        return true;
    }

    // Search in labels
    for label in &issue.labels {
        if label.to_lowercase().contains(&query_lower) {
            return true;
        }
    }

    false
}

async fn handle_list_command(labels: Vec<String>, state: &AppState) {
    let Some(ref project) = state.current_project else {
        print_colored_message("No project selected.\n", Color::DarkYellow);
        return;
    };

    let Some(ref token) = state.cached_token else {
        print_colored_message("Not authenticated. Use /login first.\n", Color::Red);
        return;
    };

    let github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.clone());

    // Use cached token for image downloads
    let github_token = state.cached_token.clone();

    print_colored_message("Fetching issues...\n", Color::DarkMagenta);

    match github.list_issues(&labels, &IssueState::Open, 20).await {
        Ok(issues) => {
            if issues.is_empty() {
                print_colored_message("No issues found with those labels.\n", Color::DarkYellow);
                return;
            }

            let auto_format = state
                .config
                .as_ref()
                .map(|c| c.auto_format_comments)
                .unwrap_or(false);

            if let Err(e) =
                tui::run_issue_browser_with_pagination(
                    issues,
                    github,
                    github_token,
                    auto_format,
                    &llm::default_endpoint(),
                    labels.clone(),
                    IssueState::Open,
                    false,
                    state.current_project_name.clone(),
                    project.local_path.clone(),
                ).await
            {
                print_colored_message(&format!("TUI error: {}\n", e), Color::Red);
            }
        }
        Err(e) => {
            print_colored_message(&format!("Failed to fetch issues: {}\n", e), Color::Red);
        }
    }
}

async fn handle_ok_command(state: &mut AppState) {
    let Some(ref session) = state.issue_session else {
        print_colored_message("No issue to create. Use /issue first.\n", Color::DarkYellow);
        return;
    };

    let Some(ref project) = state.current_project else {
        print_colored_message("No project selected.\n", Color::DarkYellow);
        return;
    };

    let Some(ref token) = state.cached_token else {
        print_colored_message("Not authenticated. Use /login first.\n", Color::Red);
        return;
    };

    let github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.clone());

    match github.create_issue(&session.issue).await {
        Ok(url) => {
            print_colored_message(&format!("Issue created: {}\n", url), Color::Green);
            state.issue_session = None;
        }
        Err(err) => {
            print_colored_message(&format!("Failed to create issue: {err}\n"), Color::Red);
        }
    }
}

fn print_help(state: &AppState) {
    if state.issue_session.is_some() {
        print_colored_message(
            "Give feedback to adapt the issue or type /ok to create it.\n",
            Color::Blue,
        );
    } else {
        let mut help = String::from("Commands:\n");
        help.push_str("  /login              - Authenticate with GitHub\n");
        help.push_str("  /logout             - Remove GitHub authentication\n");
        help.push_str("  /repository <name>  - Select a project from config\n");
        help.push_str("  /list [options]     - List issues (use --state=closed, labels, search)\n");
        help.push_str("  /issue <desc>       - Generate an issue from description\n");
        help.push_str("  /ok                 - Create the issue on GitHub\n");
        help.push_str("  /agents             - View agent sessions (Claude Code)\n");
        help.push_str("  /sessions           - List active tmux agent sessions\n");
        help.push_str("  /attach <issue#>    - Show command to attach to agent session\n");
        help.push_str("  /quit               - Exit\n");

        if let Some(ref project) = state.current_project {
            help.push_str(&format!(
                "\nCurrent project: {}/{}\n",
                project.owner, project.repo
            ));

            if !project.list_commands.is_empty() {
                help.push_str("\nList commands:\n");
                for (name, labels) in &project.list_commands {
                    help.push_str(&format!(
                        "  /{:<16} - List issues with: {}\n",
                        name,
                        labels.join(", ")
                    ));
                }
            }
        }

        if state.cached_token.is_some() {
            help.push_str("\nGitHub: logged in\n");
        } else {
            help.push_str("\nGitHub: not logged in\n");
        }

        print_colored_message(&help, Color::DarkMagenta);
    }
}

fn print_colored_message(message: &str, color: Color) {
    let mut stdout = io::stdout();
    execute!(
        stdout,
        SetForegroundColor(color),
        Print(message),
        ResetColor
    )
    .unwrap();
}

fn print_issue(issue: &IssueContent) {
    print_colored_message(
        &format!(
            "\n--- Generated Issue ---\nType: {}\nLabels: {}\nTitle: {}\n\n{}\n-----------------------\n\n",
            issue.type_,
            issue.labels.join(", "),
            issue.title,
            issue.body,
        ),
        Color::DarkYellow,
    );
}

async fn handle_agents_command(args: &str, state: &AppState) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let subcommand = parts.first().copied().unwrap_or("");
    let rest = parts.get(1..).unwrap_or(&[]).join(" ");

    match subcommand {
        "" => {
            // Open the TUI agent list view
            let Some(ref project) = state.current_project else {
                print_colored_message("No project selected. Use /repository <name> first.\n", Color::DarkYellow);
                return;
            };

            let Some(ref token) = state.cached_token else {
                print_colored_message("Not authenticated. Use /login first.\n", Color::Red);
                return;
            };

            let github = GitHubConfig::new(project.owner.clone(), project.repo.clone(), token.clone());
            let github_token = state.cached_token.clone();

            let auto_format = state
                .config
                .as_ref()
                .map(|c| c.auto_format_comments)
                .unwrap_or(false);

            // Show agent list in TUI
            if let Err(e) = tui::run_agent_browser(github, github_token, auto_format, &llm::default_endpoint()).await {
                print_colored_message(&format!("TUI error: {}\n", e), Color::Red);
            }
        }

        "list" => {
            // List sessions in text mode
            let manager = SessionManager::load();
            let sessions = manager.list();

            if sessions.is_empty() {
                print_colored_message("No agent sessions.\n", Color::DarkMagenta);
                return;
            }

            print_colored_message("Agent sessions:\n", Color::Cyan);
            for session in sessions {
                let status = match &session.status {
                    assistant::agents::AgentStatus::Running => "Running".to_string(),
                    assistant::agents::AgentStatus::Awaiting => "Awaiting input".to_string(),
                    assistant::agents::AgentStatus::Completed { exit_code } => format!("Completed ({})", exit_code),
                    assistant::agents::AgentStatus::Failed { error } => format!("Failed: {}", error),
                };
                let stats = format!("+{} -{} {} files",
                    session.stats.lines_added,
                    session.stats.lines_deleted,
                    session.stats.files_changed
                );
                let pr = session.pr_url.as_ref().map(|_| " [PR]").unwrap_or("");

                print_colored_message(
                    &format!(
                        "  {} #{} {} - {} {} {}{}\n",
                        &session.id[..8],
                        session.issue_number,
                        session.issue_title,
                        status,
                        stats,
                        session.duration_str(),
                        pr
                    ),
                    if session.is_running() { Color::Green } else { Color::White },
                );
            }
        }

        "logs" => {
            // Show logs for a session
            if rest.is_empty() {
                print_colored_message("Usage: /agents logs <session-id>\n", Color::DarkMagenta);
                return;
            }

            let manager = SessionManager::load();
            let session_id = &rest;

            // Find session by prefix match
            let session = manager.list().iter().find(|s| s.id.starts_with(session_id));

            match session {
                Some(session) => {
                    if session.log_file.exists() {
                        match std::fs::read_to_string(&session.log_file) {
                            Ok(content) => {
                                print_colored_message(
                                    &format!("--- Logs for session {} (issue #{}) ---\n", &session.id[..8], session.issue_number),
                                    Color::Cyan,
                                );
                                println!("{}", content);
                                print_colored_message("--- End of logs ---\n", Color::Cyan);
                            }
                            Err(e) => {
                                print_colored_message(&format!("Failed to read logs: {}\n", e), Color::Red);
                            }
                        }
                    } else {
                        print_colored_message("Log file not found.\n", Color::DarkYellow);
                    }
                }
                None => {
                    print_colored_message(&format!("Session '{}' not found.\n", session_id), Color::DarkYellow);
                }
            }
        }

        "kill" => {
            // Kill an agent
            if rest.is_empty() {
                print_colored_message("Usage: /agents kill <session-id>\n", Color::DarkMagenta);
                return;
            }

            let manager = SessionManager::load();
            let session_id = &rest;

            // Find session by prefix match
            let session = manager.list().iter().find(|s| s.id.starts_with(session_id));

            match session {
                Some(session) => {
                    if session.is_running() {
                        match kill_agent(&session.id) {
                            Ok(()) => {
                                print_colored_message(
                                    &format!("Killed agent {} (issue #{})\n", &session.id[..8], session.issue_number),
                                    Color::Green,
                                );
                            }
                            Err(e) => {
                                print_colored_message(&format!("Failed to kill agent: {}\n", e), Color::Red);
                            }
                        }
                    } else {
                        print_colored_message("Agent is not running.\n", Color::DarkYellow);
                    }
                }
                None => {
                    print_colored_message(&format!("Session '{}' not found.\n", session_id), Color::DarkYellow);
                }
            }
        }

        "clean" => {
            // Clean old sessions
            let mut manager = SessionManager::load();
            let before = manager.list().len();
            manager.cleanup_old_sessions(7);
            let after = manager.list().len();

            if let Err(e) = manager.save() {
                print_colored_message(&format!("Failed to save sessions: {}\n", e), Color::Red);
                return;
            }

            let removed = before - after;
            if removed > 0 {
                print_colored_message(&format!("Cleaned {} old sessions.\n", removed), Color::Green);
            } else {
                print_colored_message("No old sessions to clean.\n", Color::DarkMagenta);
            }
        }

        _ => {
            print_colored_message(
                "Usage: /agents [list|logs <id>|kill <id>|clean]\n",
                Color::DarkMagenta,
            );
        }
    }
}

fn handle_sessions_command(state: &AppState) {
    let Some(ref project_name) = state.current_project_name else {
        print_colored_message(
            "No project selected. Use /repository <name> first.\n",
            Color::DarkYellow,
        );
        return;
    };

    // List tmux sessions
    let sessions = list_tmux_sessions();
    let project_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.starts_with(project_name))
        .collect();

    if project_sessions.is_empty() {
        print_colored_message("No active agent sessions.\n", Color::DarkMagenta);
        print_colored_message(
            "Tip: Dispatch an issue with 'd' in the issue browser.\n",
            Color::DarkMagenta,
        );
        return;
    }

    print_colored_message("Active tmux sessions:\n", Color::Cyan);
    for session_name in &project_sessions {
        // Extract issue number from session name
        let status = if is_tmux_session_running(session_name) {
            "running"
        } else {
            "stopped"
        };
        print_colored_message(
            &format!("  {} ({})\n", session_name, status),
            Color::Green,
        );
    }
    print_colored_message(
        "\nUse /attach <issue-number> to join a session.\n",
        Color::DarkMagenta,
    );
}

fn handle_attach_command(args: &str, state: &AppState) {
    let Some(ref project_name) = state.current_project_name else {
        print_colored_message(
            "No project selected. Use /repository <name> first.\n",
            Color::DarkYellow,
        );
        return;
    };

    if args.is_empty() {
        // List available sessions
        let sessions = list_tmux_sessions();
        let project_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| s.starts_with(project_name))
            .collect();

        if project_sessions.is_empty() {
            print_colored_message("No active sessions to attach to.\n", Color::DarkYellow);
        } else {
            print_colored_message("Available sessions:\n", Color::Cyan);
            for s in &project_sessions {
                print_colored_message(&format!("  {}\n", s), Color::Green);
            }
            print_colored_message(
                "\nUsage: /attach <issue-number>\n",
                Color::DarkMagenta,
            );
        }
        return;
    }

    // Parse issue number
    let issue_number: u64 = match args.parse() {
        Ok(n) => n,
        Err(_) => {
            print_colored_message("Invalid issue number.\n", Color::Red);
            return;
        }
    };

    let session_name = tmux_session_name(project_name, issue_number);

    if !is_tmux_session_running(&session_name) {
        print_colored_message(
            &format!("No active session for issue #{}.\n", issue_number),
            Color::DarkYellow,
        );
        return;
    }

    // Print the command to attach
    let cmd = attach_tmux_command(&session_name);
    print_colored_message(
        &format!(
            "Run this command in your terminal to attach:\n\n  {}\n\n",
            cmd
        ),
        Color::Cyan,
    );
    print_colored_message(
        "Tip: Use Ctrl+B then D to detach from the session.\n",
        Color::DarkMagenta,
    );
}

async fn handle_feedback(
    feedback: &str,
    session: &mut IssueSession,
) -> Result<(), Box<dyn std::error::Error>> {
    session.messages.push(llm::Message {
        role: "user".to_string(),
        content: feedback.to_string(),
    });

    let response =
        llm::generate_response(&mut session.messages, &llm::default_endpoint()).await?;
    let updated_issue: IssueContent = serde_json::from_str(&response.message.content)?;
    session.issue = updated_issue;

    let serialized = serde_json::to_string(&session.issue)?;
    session.messages.push(llm::Message {
        role: "assistant".to_string(),
        content: serialized,
    });

    print_issue(&session.issue);
    Ok(())
}
