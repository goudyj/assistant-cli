//! Markdown rendering utilities for the TUI.

use ratatui::{
    style::{Modifier, Style, Color},
    text::{Line, Span},
};
use regex::Regex;

/// Parse markdown/HTML content for better terminal display
pub fn parse_markdown_content(content: &str) -> String {
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
pub fn render_markdown_line(line: &str) -> Line<'static> {
    let trimmed = line.trim();

    // Headers - differentiated by style
    if let Some(h3_content) = trimmed.strip_prefix("### ") {
        // H3: smaller, gray-cyan
        return Line::styled(
            format!("   {}", h3_content),
            Style::default().fg(Color::DarkGray),
        );
    }
    if let Some(h2_content) = trimmed.strip_prefix("## ") {
        // H2: cyan bold
        return Line::styled(
            format!("▸ {}", h2_content),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    }
    if let Some(h1_content) = trimmed.strip_prefix("# ") {
        // H1: uppercase, bright cyan, with underline effect
        return Line::styled(
            format!("═ {} ═", h1_content.to_uppercase()),
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
        && let Some(dot_pos) = trimmed.find(". ") {
            let num = &trimmed[..dot_pos];
            let content = &trimmed[dot_pos + 2..];
            return Line::from(vec![
                Span::styled(format!("  {}. ", num), Style::default().fg(Color::Yellow)),
                Span::raw(render_inline_markdown(content)),
            ]);
        }

    // [Image: ...] markers
    if trimmed.starts_with("[Image:") {
        return Line::styled(line.to_string(), Style::default().fg(Color::Magenta));
    }

    // Regular line with inline markdown (bold)
    render_line_with_bold(line)
}

/// Render inline markdown (remove ** but we can't really bold inline in ratatui easily)
pub fn render_inline_markdown(text: &str) -> String {
    // Remove ** markers for bold - terminal will show clean text
    let bold_regex = Regex::new(r"\*\*([^*]+)\*\*").unwrap();
    bold_regex.replace_all(text, "$1").to_string()
}

/// Render a line handling **bold** sections
pub fn render_line_with_bold(line: &str) -> Line<'static> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
