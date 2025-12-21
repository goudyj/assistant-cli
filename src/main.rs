use assistant::issues::{self, IssueContent};
use assistant::llm;
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Issue { description: Option<String> },
}
#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Commands::Issue { description } => {
            let desc = match description {
                Some(d) => d,
                None => {
                    println!("Please enter the issue description:");
                    let mut input = String::new();
                    io::stdin()
                        .read_line(&mut input)
                        .expect("Failed to read line");
                    input.trim().to_string()
                }
            };
            match issues::generate_issue(&desc).await {
                Ok((mut issue, mut messages)) => {
                    loop {
                        print_issue(&issue);
                        print_colored_message(
                            "Write OK if you're satisfied with the issue to create it. To edit, type your feedback. Enter sends a single line; Shift+Enter adds a blank line; send by pressing Enter twice on an empty line (double blank) or by ending a line with \\\\.\n",
                            Color::Blue,
                        );

                        let Some(user_message) = read_multiline_input() else {
                            print_colored_message(
                                "No input received (stdin closed). Stopping.\n",
                                Color::Yellow,
                            );
                            break;
                        };

                        if user_message.eq_ignore_ascii_case("OK") {
                            print_colored_message(
                                "Issue creation is not implemented yet.\n",
                                Color::Yellow,
                            );
                            break;
                        }

                        messages.push(llm::Message {
                            role: "user".to_string(),
                            content: user_message,
                        });

                        match llm::generate_response(&mut messages).await {
                            Ok(response) => {
                                match serde_json::from_str::<IssueContent>(&response.message.content) {
                                    Ok(updated_issue) => {
                                        issue = updated_issue;
                                        match serde_json::to_string(&issue) {
                                            Ok(serialized) => messages.push(llm::Message {
                                                role: "assistant".to_string(),
                                                content: serialized,
                                            }),
                                            Err(err) => {
                                                eprintln!("Error serializing issue JSON: {err:#?}");
                                                break;
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        eprintln!("Error parsing issue JSON: {err:#?}");
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                eprintln!("Error generating issue {err:#?}");
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error generating issue {err:#?}");
                }
            }
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

fn read_multiline_input() -> Option<String> {
    let mut lines = Vec::new();
    let mut blank_streak = 0;

    loop {
        let mut buf = String::new();
        let bytes_read = io::stdin().read_line(&mut buf).ok()?;
        if bytes_read == 0 {
            return if lines.is_empty() { None } else { Some(lines.join("\n")) };
        }

        let trimmed = buf.trim_end();
        if trimmed.is_empty() {
            if lines.is_empty() {
                continue;
            }
            blank_streak += 1;
            // One blank line: keep it (Shift+Enter), continue collecting.
            if blank_streak == 1 {
                lines.push(String::new());
                continue;
            }
            // Two consecutive blank lines: stop and return what we have.
            break;
        }

        // Non-empty line: reset blank streak.
        blank_streak = 0;

        // Allow continuation when the line ends with a backslash.
        if trimmed.ends_with('\\') {
            let without_slash = trimmed.trim_end_matches('\\');
            lines.push(without_slash.to_string());
            continue;
        }

        lines.push(trimmed.to_string());
        // By default, send after a normal line unless user continues with blank or backslash.
        break;
    }

    Some(lines.join("\n"))
}
