//! HTML sanitizing pipeline for emails — defence in depth:
//!
//! 1. `ammonia` removes scripts, event handlers and dangerous URLs;
//! 2. remote images are replaced by a neutral pixel (privacy: no tracking
//!    pixel, no IP address leak);
//! 3. the display happens in a `sandbox` iframe whose document embeds a CSP
//!    `default-src 'none'` — even if an escaping trick got through layers
//!    1-2, nothing can execute or load.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 1×1 grey GIF: replaces every blocked remote image.
pub const BLOCKED_PIXEL: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAMLCwgAAACH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==";

/// Fate of the remote images. Blocking is the non-negotiable default;
/// displaying is an explicit choice of the user, per message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePolicy {
    BlockRemote,
    AllowRemote,
}

pub struct Sanitized {
    pub html: String,
    pub remote_images_blocked: usize,
    pub styles_cleaned: usize,
}

pub fn sanitize(html: &str) -> Sanitized {
    sanitize_with(html, ImagePolicy::BlockRemote)
}

pub fn sanitize_with(html: &str, policy: ImagePolicy) -> Sanitized {
    let remote_images = Arc::new(AtomicUsize::new(0));
    let styles_cleaned = Arc::new(AtomicUsize::new(0));
    let images_counter = Arc::clone(&remote_images);
    let styles_counter = Arc::clone(&styles_cleaned);

    let clean = ammonia::Builder::default()
        // R3: `ammonia` removes a forbidden tag but UNWRAPS its text
        // (default). An email whose `<head><title>` repeats the subject
        // leaked it at the top of the body, duplicated. We remove the
        // CONTENT of `<title>` (script/style already are by default) — like
        // every mature client that throws the `<head>` away.
        .add_clean_content_tags(["title"])
        .add_tags(["font"])
        .add_tag_attributes("font", ["color", "face", "size"])
        .add_generic_attributes([
            // The marker of the forwarded block (PLAN-AUDIT-V2 E10, D8) —
            // inert when reading, it tells the send where the block comes from.
            "data-wind-transfert",
            "style",
            "width",
            "height",
            "align",
            "valign",
            "bgcolor",
            "border",
            "cellpadding",
            "cellspacing",
        ])
        .url_schemes(HashSet::from([
            "http", "https", "mailto", "tel", "cid", "data",
        ]))
        .attribute_filter(move |element, attribute, value| {
            filter_attribute(
                element,
                attribute,
                value,
                policy,
                &images_counter,
                &styles_counter,
            )
        })
        .clean(html)
        .to_string();

    Sanitized {
        html: clean,
        remote_images_blocked: remote_images.load(Ordering::Relaxed),
        styles_cleaned: styles_cleaned.load(Ordering::Relaxed),
    }
}

fn filter_attribute<'a>(
    element: &str,
    attribute: &str,
    value: &'a str,
    policy: ImagePolicy,
    remote_images: &AtomicUsize,
    styles_cleaned: &AtomicUsize,
) -> Option<Cow<'a, str>> {
    if element == "img" && attribute == "src" {
        let lower = value.trim().to_ascii_lowercase();
        let remote = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("//");
        if remote && policy == ImagePolicy::BlockRemote {
            remote_images.fetch_add(1, Ordering::Relaxed);
            return Some(Cow::Borrowed(BLOCKED_PIXEL));
        }
        return Some(Cow::Borrowed(value));
    }
    // `data:` is allowed for images, not for links (phishing).
    if attribute == "href" && value.trim_start().to_ascii_lowercase().starts_with("data:") {
        return None;
    }
    if attribute == "style" {
        let cleaned = clean_style(value);
        if cleaned.len() != value.len() {
            styles_cleaned.fetch_add(1, Ordering::Relaxed);
        }
        return Some(Cow::Owned(cleaned));
    }
    Some(Cow::Borrowed(value))
}

/// CSS filtering per declaration: removes any load or execution.
/// Deliberately naive (CSS escapes such as `\75rl(` would get through):
/// the iframe's CSP is the safety net (crate doc). The fidelity of `<style>`
/// blocks will come with a real CSS parser.
fn clean_style(value: &str) -> String {
    value
        .split(';')
        .filter(|declaration| {
            let compact: String = declaration
                .to_ascii_lowercase()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            !(compact.contains("url(")
                || compact.contains("expression(")
                || compact.contains("@import")
                || compact.contains("behavior:"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_script_tags_and_their_content() {
        let out = sanitize("<p>content</p><script>alert(1)</script>");
        assert!(!out.html.contains("script"));
        assert!(!out.html.contains("alert"));
        assert!(out.html.contains("content"));
    }

    /// R3 (PLAN-RETOURS-MAIL): a newsletter carries its subject in
    /// `<head><title>…</title>`. `ammonia` removes the `<title>` tag but
    /// UNWRAPS its text by default — the subject then leaked at the top of
    /// the body, duplicated (CE field: Gmail, for its part, throws the
    /// `<head>` away). Its content must disappear, tag AND text.
    #[test]
    fn drops_head_title_content_entirely() {
        let out = sanitize(
            "<html><head><title>Subject of the newsletter</title></head>\
             <body><h1>Subject of the newsletter</h1><p>body</p></body></html>",
        );
        assert!(
            !out.html.contains("<title"),
            "the title tag must go: {}",
            out.html
        );
        // The body keeps ITS title (h1); only the text of the <title> leaked.
        assert_eq!(
            out.html.matches("Subject of the newsletter").count(),
            1,
            "the text of the <title> must no longer duplicate the body: {}",
            out.html
        );
        assert!(out.html.contains("body"));
    }

    #[test]
    fn removes_event_handlers() {
        let out = sanitize(r#"<img src="data:image/gif;base64,AA==" onerror="alert(1)">"#);
        assert!(!out.html.contains("onerror"));
        assert!(!out.html.contains("alert"));
    }

    #[test]
    fn removes_javascript_links() {
        let out = sanitize(r#"<a href="javascript:alert(1)">click</a>"#);
        assert!(!out.html.contains("javascript:"));
        assert!(out.html.contains("click"));
    }

    /// PLAN-AUDIT-V2 E8: the NAMED nets of the second boundary — the HTML
    /// of a received mail can be put back into the main document (composer,
    /// signature); every classic vector has its test.
    #[test]
    fn an_svg_with_an_inline_handler_does_not_survive() {
        let out = sanitize(r#"<p>ok</p><svg onload="alert(1)"><circle r="1"/></svg>"#);
        assert!(!out.html.contains("onload"));
        assert!(!out.html.contains("<svg"));
        assert!(out.html.contains("ok"));
    }

    #[test]
    fn a_remote_srcset_does_not_survive_under_block_remote() {
        let out = sanitize(r#"<img src="cid:x" srcset="https://tracker.example/p.gif 1x">"#);
        assert!(!out.html.contains("srcset"));
        assert!(!out.html.contains("tracker.example"));
    }

    #[test]
    fn a_meta_refresh_does_not_survive() {
        let out =
            sanitize(r#"<meta http-equiv="refresh" content="0;url=https://x.example"><p>ok</p>"#);
        assert!(!out.html.contains("http-equiv"));
        assert!(!out.html.contains("x.example"));
    }

    #[test]
    fn a_base_href_does_not_survive() {
        let out = sanitize(r#"<base href="https://x.example/"><a href="/page">link</a>"#);
        assert!(!out.html.contains("<base"));
        assert!(!out.html.contains("x.example"));
    }

    /// The marker of the forwarded block (PLAN-AUDIT-V2 E10, D8) survives
    /// the boundary: a forward draft resumed later must still know where
    /// it comes from to restore its images at send time.
    #[test]
    fn the_forward_marker_survives_the_boundary() {
        let out = sanitize(r#"<div data-wind-transfert="3/42/INBOX"><p>x</p></div>"#);
        assert!(
            out.html.contains(r#"data-wind-transfert="3/42/INBOX""#),
            "{}",
            out.html
        );
    }

    #[test]
    fn blocks_remote_images_with_neutral_pixel() {
        let out = sanitize(r#"<img src="https://tracker.example.com/pixel.gif" width="1">"#);
        assert_eq!(out.remote_images_blocked, 1);
        assert!(out.html.contains(BLOCKED_PIXEL));
        assert!(!out.html.contains("tracker.example.com"));
    }

    #[test]
    fn allow_remote_keeps_images_but_still_strips_scripts() {
        let out = sanitize_with(
            r#"<img src="https://cdn.example.com/photo.jpg"><script>alert(1)</script>"#,
            ImagePolicy::AllowRemote,
        );
        assert_eq!(out.remote_images_blocked, 0);
        assert!(out.html.contains("https://cdn.example.com/photo.jpg"));
        assert!(!out.html.contains("script"));
    }

    #[test]
    fn keeps_inline_and_data_images() {
        let out = sanitize(r#"<img src="data:image/png;base64,AA==">"#);
        assert_eq!(out.remote_images_blocked, 0);
        assert!(out.html.contains("data:image/png"));
    }

    #[test]
    fn strips_css_url_loads_but_keeps_layout_declarations() {
        let out = sanitize(
            r#"<div style="background-image: url('https://x.example/bg.png'); padding: 4px">x</div>"#,
        );
        assert!(!out.html.contains("x.example"));
        assert!(out.html.contains("padding: 4px"));
        assert_eq!(out.styles_cleaned, 1);
    }

    #[test]
    fn strips_css_url_with_surrounding_whitespace() {
        let out = sanitize("<div style=\"background:\n\t url( 'https://x.example/a' )\">x</div>");
        assert!(!out.html.contains("x.example"));
    }

    /// Known and assumed limit: a CSS escape (`\75rl(` = `url(`) passes the
    /// naive filter. This test documents why layer 3 (CSP `default-src
    /// 'none'` in the iframe) is not optional.
    #[test]
    fn css_escape_bypass_passes_the_naive_filter_csp_is_the_backstop() {
        let out = sanitize(r#"<div style="background:\75rl(https://x.example/a)">x</div>"#);
        assert!(out.html.contains("x.example"));
    }

    #[test]
    fn removes_data_links_but_not_data_images() {
        let out = sanitize(r#"<a href="data:text/html;base64,PHNjcmlwdD4=">x</a>"#);
        assert!(!out.html.contains("href"));
    }

    #[test]
    fn keeps_table_layout_used_by_newsletters() {
        let out = sanitize(
            r##"<table width="600" bgcolor="#ffffff" cellpadding="0"><tbody><tr><td align="center" style="color: #333">content</td></tr></tbody></table>"##,
        );
        assert!(out.html.contains(r#"width="600""#));
        assert!(out.html.contains(r#"align="center""#));
        assert!(out.html.contains("color: #333"));
    }
}
