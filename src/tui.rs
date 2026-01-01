use crate::github::{GitHubConfig, IssueDetail, IssueSummary};
use crate::llm;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::ImageReader;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use regex::Regex;
use std::io::{self, Cursor};

/// View state for the TUI
pub enum TuiView {
    List,
    Detail(IssueDetail),
    AddComment { issue: IssueDetail, input: String },
}

/// Main TUI state
pub struct IssueBrowser {
    pub issues: Vec<IssueSummary>,
    pub list_state: ListState,
    pub view: TuiView,
    pub scroll_offset: u16,
    pub should_quit: bool,
    pub github: GitHubConfig,
    pub github_token: Option<String>,
    pub auto_format: bool,
    pub llm_endpoint: String,
    pub status_message: Option<String>,
    pub current_images: Vec<String>,
    pub current_image_index: usize,
}

impl IssueBrowser {
    pub fn new(
        issues: Vec<IssueSummary>,
        github: GitHubConfig,
        github_token: Option<String>,
        auto_format: bool,
        llm_endpoint: String,
    ) -> Self {
        let mut list_state = ListState::default();
        if !issues.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            issues,
            list_state,
            view: TuiView::List,
            scroll_offset: 0,
            should_quit: false,
            github,
            github_token,
            auto_format,
            llm_endpoint,
            status_message: None,
            current_images: Vec::new(),
            current_image_index: 0,
        }
    }

    /// Extract image URLs from issue content
    pub fn extract_images_from_issue(&mut self, issue: &IssueDetail) {
        let mut images = Vec::new();

        // Extract from body
        if let Some(ref body) = issue.body {
            images.extend(extract_image_urls(body));
        }

        // Extract from comments
        for comment in &issue.comments {
            images.extend(extract_image_urls(&comment.body));
        }

        self.current_images = images;
        self.current_image_index = 0;
    }

    pub fn selected_issue(&self) -> Option<&IssueSummary> {
        self.list_state.selected().and_then(|i| self.issues.get(i))
    }

    pub fn next(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.issues.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.issues.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.issues.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

/// Run the TUI application
pub async fn run_issue_browser(
    issues: Vec<IssueSummary>,
    github: GitHubConfig,
    github_token: Option<String>,
    auto_format: bool,
    llm_endpoint: &str,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // No mouse capture to allow text selection / copy-paste
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut browser = IssueBrowser::new(
        issues,
        github,
        github_token,
        auto_format,
        llm_endpoint.to_string(),
    );

    while !browser.should_quit {
        terminal.draw(|f| draw_ui(f, &mut browser))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(&mut browser, key.code).await;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn draw_ui(f: &mut Frame, browser: &mut IssueBrowser) {
    let image_count = browser.current_images.len();
    match &browser.view {
        TuiView::List => draw_list_view(f, browser),
        TuiView::Detail(issue) => {
            draw_detail_view(f, f.area(), issue, browser.scroll_offset, image_count);
        }
        TuiView::AddComment { issue, input } => {
            // Split screen: issue on top (75%), comment input at bottom (25%)
            let chunks = Layout::vertical([Constraint::Percentage(75), Constraint::Percentage(25)])
                .split(f.area());

            draw_detail_view(f, chunks[0], issue, browser.scroll_offset, image_count);
            draw_comment_input(f, chunks[1], input, browser.status_message.as_deref());
        }
    }
}

fn draw_list_view(f: &mut Frame, browser: &mut IssueBrowser) {
    let items: Vec<ListItem> = browser
        .issues
        .iter()
        .map(|issue| {
            let labels_str = if issue.labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", issue.labels.join(", "))
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("#{:<5}", issue.number),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::raw(&issue.title),
                Span::styled(labels_str, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Issues (↑↓ navigate, Enter view, Esc quit) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, f.area(), &mut browser.list_state);
}

fn draw_detail_view(
    f: &mut Frame,
    area: Rect,
    issue: &IssueDetail,
    scroll: u16,
    image_count: usize,
) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&issue.title),
        ]),
        Line::from(vec![
            Span::styled("URL: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(&issue.html_url, Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("Labels: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(issue.labels.join(", ")),
        ]),
        Line::from(""),
        Line::styled("─── Body ───", Style::default().fg(Color::Yellow)),
    ];

    if let Some(ref body) = issue.body {
        let parsed_body = parse_markdown_content(body);
        for line in parsed_body.lines() {
            lines.push(render_markdown_line(line));
        }
    } else {
        lines.push(Line::styled(
            "(no description)",
            Style::default().fg(Color::DarkGray),
        ));
    }

    if !issue.comments.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!("─── Comments ({}) ───", issue.comments.len()),
            Style::default().fg(Color::Yellow),
        ));

        for comment in &issue.comments {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(&comment.author, Style::default().fg(Color::Green)),
                Span::raw(" - "),
                Span::styled(
                    format_date(&comment.created_at),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            let parsed_comment = parse_markdown_content(&comment.body);
            for line in parsed_comment.lines() {
                lines.push(Line::from(format!("  {}", line)));
            }
        }
    }

    let title = if image_count > 0 {
        format!(
            " #{} │ o open │ c comment │ i/O image [{}/{}] │ ↑↓ scroll │ Esc ",
            issue.number, 1, image_count
        )
    } else {
        format!(" #{} │ o open │ c comment │ ↑↓ scroll │ Esc ", issue.number)
    };

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}

/// Parse markdown/HTML content for better terminal display
fn parse_markdown_content(content: &str) -> String {
    let mut result = content.to_string();

    // Replace <img> tags with [Image: alt or url]
    let img_regex =
        Regex::new(r#"<img[^>]*(?:alt="([^"]*)")?[^>]*src="([^"]*)"[^>]*/?\s*>"#).unwrap();
    result = img_regex
        .replace_all(&result, |caps: &regex::Captures| {
            let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let src = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !alt.is_empty() && alt != "Image" {
                format!("[Image: {}]", alt)
            } else {
                format!("[Image: {}]", src)
            }
        })
        .to_string();

    // Also handle img tags where src comes before alt
    let img_regex2 =
        Regex::new(r#"<img[^>]*src="([^"]*)"[^>]*(?:alt="([^"]*)")?[^>]*/?\s*>"#).unwrap();
    result = img_regex2
        .replace_all(&result, |caps: &regex::Captures| {
            let src = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let alt = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !alt.is_empty() && alt != "Image" {
                format!("[Image: {}]", alt)
            } else {
                format!("[Image: {}]", src)
            }
        })
        .to_string();

    result
}

/// Render a markdown line with basic styling
fn render_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();

    // Headers - differentiated by style
    if trimmed.starts_with("### ") {
        // H3: smaller, gray-cyan
        return Line::styled(
            format!("   {}", &trimmed[4..]),
            Style::default().fg(Color::DarkGray),
        );
    }
    if trimmed.starts_with("## ") {
        // H2: cyan bold
        return Line::styled(
            format!("▸ {}", &trimmed[3..]),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    }
    if trimmed.starts_with("# ") {
        // H1: uppercase, bright cyan, with underline effect
        return Line::styled(
            format!("═ {} ═", trimmed[2..].to_uppercase()),
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        );
    }

    // Code blocks
    if trimmed.starts_with("```") {
        return Line::styled(
            "─────────────────────".to_string(),
            Style::default().fg(Color::DarkGray),
        );
    }

    // Bullet points
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let content = &trimmed[2..];
        return Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::Yellow)),
            Span::raw(render_inline_markdown(content)),
        ]);
    }

    // Numbered lists
    if trimmed.len() > 2
        && trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
    {
        if let Some(dot_pos) = trimmed.find(". ") {
            let num = &trimmed[..dot_pos];
            let content = &trimmed[dot_pos + 2..];
            return Line::from(vec![
                Span::styled(format!("  {}. ", num), Style::default().fg(Color::Yellow)),
                Span::raw(render_inline_markdown(content)),
            ]);
        }
    }

    // [Image: ...] markers
    if trimmed.starts_with("[Image:") {
        return Line::styled(line.to_string(), Style::default().fg(Color::Magenta));
    }

    // Regular line with inline markdown (bold)
    render_line_with_bold(line)
}

/// Render inline markdown (remove ** but we can't really bold inline in ratatui easily)
fn render_inline_markdown(text: &str) -> String {
    // Remove ** markers for bold - terminal will show clean text
    let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    bold_regex.replace_all(text, "$1").to_string()
}

/// Render a line handling **bold** sections
fn render_line_with_bold(line: &str) -> Line<'static> {
    let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();

    // Check if line contains bold markers
    if !line.contains("**") {
        return Line::from(line.to_string());
    }

    let mut spans = Vec::new();
    let mut last_end = 0;

    for cap in bold_regex.captures_iter(line) {
        let full_match = cap.get(0).unwrap();
        let bold_text = cap.get(1).unwrap();

        // Add text before the bold part
        if full_match.start() > last_end {
            spans.push(Span::raw(line[last_end..full_match.start()].to_string()));
        }

        // Add bold text
        spans.push(Span::styled(
            bold_text.as_str().to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));

        last_end = full_match.end();
    }

    // Add remaining text
    if last_end < line.len() {
        spans.push(Span::raw(line[last_end..].to_string()));
    }

    Line::from(spans)
}

fn draw_comment_input(f: &mut Frame, area: Rect, input: &str, status: Option<&str>) {
    let title = if let Some(msg) = status {
        format!(" {} ", msg)
    } else {
        " Add Comment (Enter send, Esc cancel) ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));

    let display_text = if input.is_empty() {
        "Type your comment here..."
    } else {
        input
    };

    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    let paragraph = Paragraph::new(display_text)
        .block(block)
        .style(style)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn format_date(date_str: &str) -> String {
    // Simple date formatting: take first 10 chars if available
    if date_str.len() >= 10 {
        date_str[..10].to_string()
    } else {
        date_str.to_string()
    }
}

async fn handle_key_event(browser: &mut IssueBrowser, key: KeyCode) {
    match &mut browser.view {
        TuiView::List => match key {
            KeyCode::Esc | KeyCode::Char('q') => browser.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') => browser.next(),
            KeyCode::Up | KeyCode::Char('k') => browser.previous(),
            KeyCode::Enter => {
                if let Some(issue) = browser.selected_issue() {
                    let number = issue.number;
                    if let Ok(detail) = browser.github.get_issue(number).await {
                        browser.extract_images_from_issue(&detail);
                        browser.view = TuiView::Detail(detail);
                        browser.scroll_offset = 0;
                    }
                }
            }
            _ => {}
        },
        TuiView::Detail(issue) => match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                browser.view = TuiView::List;
                browser.scroll_offset = 0;
                browser.current_images.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => browser.scroll_offset += 1,
            KeyCode::Up | KeyCode::Char('k') => {
                browser.scroll_offset = browser.scroll_offset.saturating_sub(1);
            }
            KeyCode::Char('c') => {
                let issue_clone = issue.clone();
                browser.view = TuiView::AddComment {
                    issue: issue_clone,
                    input: String::new(),
                };
                browser.status_message = None;
            }
            KeyCode::Char('o') => {
                // Open issue in browser
                open_url(&issue.html_url);
                browser.status_message = Some("Opened in browser".to_string());
            }
            KeyCode::Char('O') => {
                // Open current image in browser
                if !browser.current_images.is_empty() {
                    let url = &browser.current_images[browser.current_image_index];
                    open_url(url);
                    browser.status_message = Some("Image opened in browser".to_string());
                    // Cycle to next image
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images".to_string());
                }
            }
            KeyCode::Char('i') => {
                // Show image in terminal if available
                if !browser.current_images.is_empty() {
                    let url = browser.current_images[browser.current_image_index].clone();
                    let token = browser.github_token.clone();
                    if let Err(e) = display_image(&url, token.as_deref()).await {
                        browser.status_message = Some(format!("Image error: {}", e));
                    }
                    // Cycle to next image for next press
                    browser.current_image_index =
                        (browser.current_image_index + 1) % browser.current_images.len();
                } else {
                    browser.status_message = Some("No images in this issue".to_string());
                }
            }
            _ => {}
        },
        TuiView::AddComment { issue, input } => match key {
            KeyCode::Esc => {
                let number = issue.number;
                if let Ok(detail) = browser.github.get_issue(number).await {
                    browser.view = TuiView::Detail(detail);
                } else {
                    browser.view = TuiView::List;
                }
                browser.status_message = None;
            }
            KeyCode::Enter => {
                if !input.is_empty() {
                    let comment_body = if browser.auto_format {
                        browser.status_message = Some("Formatting...".to_string());
                        format_comment_with_llm(input, &browser.llm_endpoint)
                            .await
                            .unwrap_or_else(|_| input.clone())
                    } else {
                        input.clone()
                    };

                    browser.status_message = Some("Sending...".to_string());
                    let number = issue.number;
                    if browser
                        .github
                        .add_comment(number, &comment_body)
                        .await
                        .is_ok()
                    {
                        // Reload issue to show new comment
                        if let Ok(detail) = browser.github.get_issue(number).await {
                            browser.view = TuiView::Detail(detail);
                        } else {
                            browser.view = TuiView::List;
                        }
                    } else {
                        browser.view = TuiView::List;
                    }
                    browser.status_message = None;
                }
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Char(c) => {
                input.push(c);
            }
            _ => {}
        },
    }
}

async fn format_comment_with_llm(
    comment: &str,
    endpoint: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut messages = vec![
        llm::Message {
            role: "system".to_string(),
            content: "You are a writing assistant. Correct grammar, fix typos, and improve clarity of the following comment for a GitHub issue. Keep it concise and professional. Return only the corrected text, no explanations or quotes.".to_string(),
        },
        llm::Message {
            role: "user".to_string(),
            content: comment.to_string(),
        },
    ];

    let response = llm::generate_response(&mut messages, endpoint).await?;
    Ok(response.message.content.trim().to_string())
}

/// Extract image URLs from content (supports HTML img tags and markdown images)
fn extract_image_urls(content: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // HTML img tags: <img src="..." />
    let img_regex = Regex::new(r#"<img[^>]*src="([^"]+)"[^>]*>"#).unwrap();
    for cap in img_regex.captures_iter(content) {
        if let Some(url) = cap.get(1) {
            urls.push(url.as_str().to_string());
        }
    }

    // Markdown images: ![alt](url)
    let md_regex = Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap();
    for cap in md_regex.captures_iter(content) {
        if let Some(url) = cap.get(1) {
            urls.push(url.as_str().to_string());
        }
    }

    urls
}

/// Display an image in the terminal using ratatui-image
async fn display_image(
    url: &str,
    github_token: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Download the image with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Add GitHub token for private repo images
    let mut request = client.get(url);
    if url.contains("github.com") || url.contains("githubusercontent.com") {
        if let Some(token) = github_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(format!("Failed to download: HTTP {}", response.status()).into());
    }

    let bytes = response.bytes().await?;

    // Decode the image
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()?;

    // Create picker to detect terminal protocol (must be done before entering alternate screen)
    // Temporarily exit raw mode for protocol detection
    disable_raw_mode()?;
    let picker = Picker::from_query_stdio()?;
    enable_raw_mode()?;

    // Create image protocol
    let mut image_state = picker.new_resize_protocol(img);

    // Show image in a dedicated view (handles its own event loop)
    show_image_view(&mut image_state, url)?;

    Ok(())
}

/// Show image in a fullscreen ratatui view
fn show_image_view(
    image_state: &mut StatefulProtocol,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stdout = io::stdout();

    // Create a new terminal for the image view
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let area = f.area();

            // Leave space for URL and instructions at top
            let chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(area);

            // Header with URL and instructions
            let header = Paragraph::new(format!(
                "{}\n\nPress any key to return, 'b' to open in browser",
                url
            ))
            .style(Style::default().fg(Color::DarkGray));
            f.render_widget(header, chunks[0]);

            // Image widget
            let image_widget = StatefulImage::default();
            f.render_stateful_widget(image_widget, chunks[1], image_state);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Char('b') {
                        let _ = open::that(url);
                    }
                    break;
                }
            }
        }
    }

    // Clear terminal before returning
    terminal.clear()?;

    Ok(())
}

/// Open a URL in the default browser
fn open_url(url: &str) {
    let _ = open::that(url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_image_urls_from_html() {
        let content = r#"Some text <img src="https://example.com/image.png" /> more text"#;
        let urls = extract_image_urls(content);
        assert_eq!(urls, vec!["https://example.com/image.png"]);
    }

    #[test]
    fn extract_image_urls_from_markdown() {
        let content = "Check this ![screenshot](https://example.com/shot.jpg) out";
        let urls = extract_image_urls(content);
        assert_eq!(urls, vec!["https://example.com/shot.jpg"]);
    }

    #[test]
    fn extract_image_urls_mixed_content() {
        let content = r#"
            ![First](https://a.com/1.png)
            <img src="https://b.com/2.png" alt="Second" />
            ![Third](https://c.com/3.png)
        "#;
        let urls = extract_image_urls(content);
        assert_eq!(urls.len(), 3);
        assert!(urls.contains(&"https://a.com/1.png".to_string()));
        assert!(urls.contains(&"https://b.com/2.png".to_string()));
        assert!(urls.contains(&"https://c.com/3.png".to_string()));
    }

    #[test]
    fn extract_image_urls_empty_content() {
        let urls = extract_image_urls("No images here");
        assert!(urls.is_empty());
    }

    #[test]
    fn parse_markdown_content_replaces_img_tag() {
        // Img tags are replaced with readable markers
        let content = r#"<img src="https://example.com/img.png" />"#;
        let result = parse_markdown_content(content);
        assert!(result.contains("[Image:"));
        assert!(result.contains("example.com"));
    }

    #[test]
    fn parse_markdown_content_preserves_text() {
        let content = "Some text before <img src=\"https://example.com/img.png\" /> and after";
        let result = parse_markdown_content(content);
        assert!(result.contains("Some text before"));
        assert!(result.contains("and after"));
        assert!(result.contains("[Image:"));
    }

    #[test]
    fn render_inline_markdown_removes_bold() {
        assert_eq!(render_inline_markdown("**bold** text"), "bold text");
        assert_eq!(
            render_inline_markdown("normal **bold** normal"),
            "normal bold normal"
        );
        assert_eq!(render_inline_markdown("no formatting"), "no formatting");
    }

    #[test]
    fn render_inline_markdown_multiple_bold() {
        assert_eq!(
            render_inline_markdown("**first** and **second**"),
            "first and second"
        );
    }
}
