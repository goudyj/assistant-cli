use assistant::auth::{self, DeviceFlowAuth};
use assistant::config::{self, Config, ProjectConfig};
use assistant::github::GitHubConfig;
use assistant::issues::{self, IssueContent};
use assistant::llm;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use dialoguer::{theme::ColorfulTheme, Select};
use reedline::{DefaultPrompt, Reedline, Signal};
use std::io;

struct AppState {
    config: Option<Config>,
    current_project: Option<ProjectConfig>,
    current_project_name: Option<String>,
    issue_session: Option<IssueSession>,
}

struct IssueSession {
    issue: IssueContent,
    messages: Vec<llm::Message>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let mut state = AppState {
        config: None,
        current_project: None,
        current_project_name: None,
        issue_session: None,
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

            if let Some(ref last_project) = cfg.last_project {
                if let Some(project) = cfg.get_project(last_project) {
                    print_colored_message(
                        &format!("Restored project: {}/{}\n", project.owner, project.repo),
                        Color::Green,
                    );
                    state.current_project = Some(project.clone());
                    state.current_project_name = Some(last_project.clone());
                }
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

    if auth::is_logged_in() {
        print_colored_message("GitHub: logged in\n", Color::Green);
    } else {
        print_colored_message("GitHub: not logged in. Use /login to authenticate.\n", Color::DarkYellow);
    }

    print_colored_message(
        "Commands: /login, /repository <name>, /issue <desc>, /ok, /quit\n",
        Color::DarkMagenta,
    );

    let mut rl = Reedline::create();
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
                    handle_login_command(&state).await;
                }

                "/logout" => {
                    handle_logout_command();
                }

                "/repository" | "/repo" => {
                    handle_repository_command(rest, &mut state);
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

                _ => print_colored_message("Unknown command. Type /help.\n", Color::DarkMagenta),
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

async fn handle_login_command(state: &AppState) {
    if auth::is_logged_in() {
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
            print_colored_message("Successfully logged in to GitHub!\n", Color::Green);
        }
        Err(e) => {
            print_colored_message(&format!("Authentication failed: {}\n", e), Color::Red);
        }
    }
}

fn handle_logout_command() {
    match auth::delete_token() {
        Ok(_) => print_colored_message("Logged out from GitHub.\n", Color::Green),
        Err(e) => print_colored_message(&format!("Logout failed: {}\n", e), Color::Red),
    }
}

fn handle_repository_command(name: &str, state: &mut AppState) {
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

async fn handle_ok_command(state: &mut AppState) {
    let Some(ref session) = state.issue_session else {
        print_colored_message("No issue to create. Use /issue first.\n", Color::DarkYellow);
        return;
    };

    let Some(ref project) = state.current_project else {
        print_colored_message("No project selected.\n", Color::DarkYellow);
        return;
    };

    let github = match GitHubConfig::from_keyring(project.owner.clone(), project.repo.clone()) {
        Ok(cfg) => cfg,
        Err(e) => {
            print_colored_message(&format!("{}\n", e), Color::Red);
            return;
        }
    };

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
        help.push_str("  /issue <desc>       - Generate an issue from description\n");
        help.push_str("  /ok                 - Create the issue on GitHub\n");
        help.push_str("  /quit               - Exit\n");

        if let Some(ref project) = state.current_project {
            help.push_str(&format!(
                "\nCurrent project: {}/{}\n",
                project.owner, project.repo
            ));
        }

        if auth::is_logged_in() {
            help.push_str("GitHub: logged in\n");
        } else {
            help.push_str("GitHub: not logged in\n");
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
