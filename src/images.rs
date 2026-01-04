//! Image URL extraction utilities.

use regex::Regex;

/// Extract image URLs from content (supports HTML img tags and markdown images)
pub fn extract_image_urls(content: &str) -> Vec<String> {
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
![First](https://example.com/first.png)
<img src="https://example.com/second.png" />
"#;
        let urls = extract_image_urls(content);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com/first.png".to_string()));
        assert!(urls.contains(&"https://example.com/second.png".to_string()));
    }

    #[test]
    fn extract_image_urls_empty_content() {
        let urls = extract_image_urls("No images here");
        assert!(urls.is_empty());
    }
}
