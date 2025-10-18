use ammonia::{Builder, clean};
use std::collections::HashSet;

/// Sanitize HTML content to prevent XSS attacks
/// This function removes dangerous tags and attributes while preserving safe HTML
pub fn sanitize_html(html: &str) -> String {
    // Use ammonia's default settings which are already quite restrictive
    clean(html)
}

/// Sanitize HTML with custom configuration for email display
/// Allows more tags and attributes suitable for email content
pub fn sanitize_email_html(html: &str) -> String {
    let mut builder = Builder::default();
    
    // Allow safe HTML tags commonly used in emails
    let allowed_tags = vec![
        "a", "abbr", "acronym", "b", "blockquote", "br", "caption", "cite", "code",
        "col", "colgroup", "dd", "del", "details", "div", "dl", "dt", "em", "figcaption",
        "figure", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "i", "img", "ins", "kbd",
        "li", "mark", "ol", "p", "pre", "q", "s", "samp", "small", "span", "strike",
        "strong", "sub", "summary", "sup", "table", "tbody", "td", "tfoot", "th", "thead",
        "time", "tr", "u", "ul", "var",
    ];
    
    builder.tags(allowed_tags.into_iter().collect::<HashSet<_>>());
    
    // Allow href for links (but ammonia will still validate URLs)
    builder.link_rel(Some("noopener noreferrer"));
    
    // Allow data URIs for images (base64 encoded)
    builder.url_schemes(vec!["http", "https", "mailto", "data"].into_iter().collect());
    
    // Clean the HTML
    builder.clean(html).to_string()
}

/// Sanitize HTML and strip all tags, leaving only text
/// Useful for generating plain text previews
pub fn strip_html_tags(html: &str) -> String {
    let mut builder = Builder::default();
    builder.tags(HashSet::new()); // No tags allowed
    builder.clean(html).to_string()
}

/// Sanitize HTML for storage in database
/// This is the primary function to use when storing user-generated HTML
pub fn sanitize_for_storage(html: &str) -> String {
    sanitize_email_html(html)
}

/// Sanitize HTML for display to users
/// This is the primary function to use when displaying HTML to users
pub fn sanitize_for_display(html: &str) -> String {
    sanitize_email_html(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_removes_script_tags() {
        let malicious = "<p>Hello</p><script>alert('XSS')</script>";
        let sanitized = sanitize_html(malicious);
        assert!(!sanitized.contains("<script>"));
        assert!(!sanitized.contains("alert"));
        assert!(sanitized.contains("<p>Hello</p>"));
    }

    #[test]
    fn test_sanitize_removes_onclick() {
        let malicious = "<a href='#' onclick='alert(1)'>Click</a>";
        let sanitized = sanitize_html(malicious);
        assert!(!sanitized.contains("onclick"));
        assert!(sanitized.contains("<a"));
    }

    #[test]
    fn test_sanitize_removes_iframe() {
        let malicious = "<p>Test</p><iframe src='evil.com'></iframe>";
        let sanitized = sanitize_html(malicious);
        assert!(!sanitized.contains("<iframe"));
        assert!(sanitized.contains("<p>Test</p>"));
    }

    #[test]
    fn test_sanitize_allows_safe_html() {
        let safe = "<p>Hello <strong>world</strong></p><a href='https://example.com'>Link</a>";
        let sanitized = sanitize_html(safe);
        assert!(sanitized.contains("<p>"));
        assert!(sanitized.contains("<strong>"));
        assert!(sanitized.contains("<a href"));
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<p>Hello <strong>world</strong></p>";
        let stripped = strip_html_tags(html);
        assert_eq!(stripped.trim(), "Hello world");
        assert!(!stripped.contains("<"));
    }

    #[test]
    fn test_sanitize_email_html_allows_tables() {
        let email_html = "<table><tr><td>Cell</td></tr></table>";
        let sanitized = sanitize_email_html(email_html);
        assert!(sanitized.contains("<table>"));
        assert!(sanitized.contains("<tr>"));
        assert!(sanitized.contains("<td>"));
    }

    #[test]
    fn test_sanitize_removes_javascript_protocol() {
        let malicious = "<a href='javascript:alert(1)'>Click</a>";
        let sanitized = sanitize_html(malicious);
        assert!(!sanitized.contains("javascript:"));
    }
}
