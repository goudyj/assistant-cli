use assistant::issues::{self, IssueContent};
use assistant::llm;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use reedline::{DefaultPrompt, Reedline, Signal};
use std::io;

#[tokio::main]
async fn main() {
    let mut rl = Reedline::create();
    let prompt = DefaultPrompt::default();
    print_colored_message(
        "Commands: /issue <desc>, /ok, /quit, or type feedback to refine the current issue.\n",
        Color::Blue,
    );

    let mut session: Option<IssueSession> = None;

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
                "/issue" => {
                    if rest.is_empty() {
                        print_colored_message("Usage: /issue <description>\n", Color::Yellow);
                        continue;
                    }
                    match issues::generate_issue(rest).await {
                        Ok((issue, messages)) => {
                            session = Some(IssueSession { issue, messages });
                            if let Some(s) = &session {
                                print_issue(&s.issue);
                            }
                        }
                        Err(err) => eprintln!("Error generating issue {err:#?}"),
                    }
                }
                "/ok" => {
                    print_colored_message(
                        "Issue creation is not implemented yet.\n",
                        Color::Yellow,
                    );
                    session = None;
                }
                "/help" => {
                    print_colored_message(
                        "Commands: /issue <desc>, /ok, /quit. Type feedback to refine the current issue.\n",
                        Color::Blue,
                    );
                }
                _ => print_colored_message("Unknown command.\n", Color::Yellow),
            }
            continue;
        }

        if let Some(s) = session.as_mut() {
            if let Err(err) = handle_feedback(&line, s).await {
                eprintln!("Error updating issue {err:#?}");
                session = None;
            }
        } else {
            print_colored_message(
                "No active issue. Start one with /issue <description>.\n",
                Color::Yellow,
            );
        }
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
            "Here is the issue that would be generated\n\nType: {}\nLabels: {}\nTitle: {}\nBody: {}\n\n",
            issue.type_,
            issue.labels.join(", "),
            issue.title,
            issue.body,
        ),
        Color::Green,
    );
}

struct IssueSession {
    issue: IssueContent,
    messages: Vec<llm::Message>,
}

async fn handle_feedback(
    feedback: &str,
    session: &mut IssueSession,
) -> Result<(), Box<dyn std::error::Error>> {
    session.messages.push(llm::Message {
        role: "user".to_string(),
        content: feedback.to_string(),
    });

    let response = llm::generate_response(&mut session.messages).await?;
    let updated_issue: IssueContent = serde_json::from_str(&response.message.content)?;
    session.issue = updated_issue;

    // Keep the assistant reply in the history so the model has context.
    let serialized = serde_json::to_string(&session.issue)?;
    session.messages.push(llm::Message {
        role: "assistant".to_string(),
        content: serialized,
    });

    print_issue(&session.issue);
    Ok(())
}
