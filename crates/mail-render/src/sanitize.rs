//! HTML sanitizing pipeline for emails — defence in depth:
//!
//! 1. `ammonia` removes scripts, event handlers and dangerous URLs;
//! 2. remote images are replaced by a neutral pixel (privacy: no tracking
//!    pixel, no IP address leak);
//! 3. the display happens in a `sandbox` iframe whose document embeds a CSP
//!    `default-src 'none'` — even if an escaping trick got through layers
//!    1-2, nothing can execute or load.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// 1×1 grey GIF: replaces every blocked remote image.
pub const BLOCKED_PIXEL: &str =
    "data:image/gif;base64,R0lGODlhAQABAIAAAMLCwgAAACH5BAAAAAAALAAAAAABAAEAAAICRAEAOw==";

static NEXT_IMAGE: AtomicUsize = AtomicUsize::new(1);

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
    pub image_sources: BTreeMap<String, String>,
}

pub fn sanitize(html: &str) -> Sanitized {
    sanitize_with(html, ImagePolicy::BlockRemote)
}

pub fn sanitize_for_composer(html: &str) -> Sanitized {
    sanitize_policy(html, ImagePolicy::BlockRemote, true, None)
}

pub fn sanitize_composition(html: &str, images: &BTreeMap<String, String>) -> Sanitized {
    sanitize_policy(html, ImagePolicy::AllowRemote, true, Some(images))
}

pub fn sanitize_with(html: &str, policy: ImagePolicy) -> Sanitized {
    sanitize_policy(html, policy, false, None)
}

fn sanitize_policy(
    html: &str,
    policy: ImagePolicy,
    composer: bool,
    images: Option<&BTreeMap<String, String>>,
) -> Sanitized {
    let remote_images = Arc::new(AtomicUsize::new(0));
    let styles_cleaned = Arc::new(AtomicUsize::new(0));
    let images_counter = Arc::clone(&remote_images);
    let styles_counter = Arc::clone(&styles_cleaned);
    let image_sources = Arc::new(Mutex::new(BTreeMap::new()));
    let sources = Arc::clone(&image_sources);
    let images = images.cloned();

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
            if composer {
                if element == "img" && attribute == "src" {
                    if policy == ImagePolicy::AllowRemote {
                        if let Some(url) = images.as_ref().and_then(|images| images.get(value)) {
                            return remote_url(url).map(|url| Cow::Owned(url.into_owned()));
                        }
                    } else if let Some(url) = remote_url(value) {
                        let id = NEXT_IMAGE.fetch_add(1, Ordering::Relaxed);
                        let placeholder = format!("{BLOCKED_PIXEL}#wind-image-{id}");
                        sources
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .insert(placeholder.clone(), url.into_owned());
                        images_counter.fetch_add(1, Ordering::Relaxed);
                        return Some(Cow::Owned(placeholder));
                    }
                    // Input cannot introduce a reference into another preparation's map.
                    if value.starts_with(BLOCKED_PIXEL) {
                        return Some(Cow::Borrowed(BLOCKED_PIXEL));
                    }
                }
                if attribute == "style" {
                    let clean = crate::style::clean_style(value);
                    if clean != value {
                        styles_counter.fetch_add(1, Ordering::Relaxed);
                    }
                    return Some(Cow::Owned(clean));
                }
                if !editor_attribute_allowed(element, attribute, value) {
                    return None;
                }
            }
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
        image_sources: image_sources
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
    }
}

/// A remote image address, always over TLS: granting remote images is
/// granting HTTPS (audit 2026-09-01), so a cleartext `http://` is upgraded
/// here rather than left in the document for the iframe's CSP to break
/// (field 2026-09-07: three of a newsletter's five images were cleartext).
fn remote_url(value: &str) -> Option<Cow<'_, str>> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("https://") {
        Some(Cow::Borrowed(value))
    } else if lower.starts_with("http://") {
        Some(Cow::Owned(format!("https://{}", &value["http://".len()..])))
    } else if value.starts_with("//") {
        Some(Cow::Owned(format!("https:{value}")))
    } else {
        None
    }
}

fn editor_attribute_allowed(element: &str, attribute: &str, value: &str) -> bool {
    let cap = match attribute {
        "width" | "height" => 1600.0,
        "cellpadding" | "cellspacing" => 64.0,
        "border" => 16.0,
        "colspan" | "rowspan" => 100.0,
        "size" if element == "font" => 7.0,
        _ => return true,
    };
    let value = value.trim();
    let (number, cap) = if let Some(number) = value.strip_suffix('%') {
        if !matches!(attribute, "width" | "height") {
            return false;
        }
        (number, 100.0)
    } else {
        (value, cap)
    };
    number
        .parse::<f32>()
        .is_ok_and(|number| number.is_finite() && (0.0..=cap).contains(&number))
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
        let remote = !(lower.starts_with("data:image/") || lower.starts_with("cid:"));
        if remote {
            if policy == ImagePolicy::AllowRemote
                && let Some(url) = remote_url(value)
            {
                return Some(url);
            }
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

/// Reading-only cleanup; escaped loads are blocked by the iframe's CSP.
/// Editable documents use the token/value policy in `style` instead.
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

    /// Field 2026-09-07 (lot 4 STOP 2): a newsletter carried three of its
    /// five images over `http://`; kept as-is under "allow", they hit the
    /// iframe's `img-src https:` and showed as broken icons. Granting remote
    /// images is granting HTTPS (audit 2026-09-01): the cleartext address
    /// is upgraded, never fetched in clear, never left broken.
    #[test]
    fn a_cleartext_image_is_upgraded_to_https_when_remote_images_are_allowed() {
        let out = sanitize_with(
            r#"<img src="http://cdn.example.net/a/206x51.png" alt="logo"><img src="https://r.example.com/b.png">"#,
            ImagePolicy::AllowRemote,
        );
        assert!(
            out.html
                .contains(r#"src="https://cdn.example.net/a/206x51.png""#),
            "{}",
            out.html
        );
        assert!(!out.html.contains("http://"), "{}", out.html);
        assert!(out.html.contains(r#"src="https://r.example.com/b.png""#));
        assert_eq!(out.remote_images_blocked, 0);
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

    #[test]
    fn received_html_cannot_supply_forward_authority() {
        let out = sanitize(r#"<div data-wind-transfert="3/42/INBOX"><p>x</p></div>"#);
        assert!(!out.html.contains("data-wind-transfert"), "{}", out.html);
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

#[cfg(test)]
mod composer_tests {
    #[test]
    fn protocol_relative_images_stay_blocked_while_editing_and_roundtrip_as_https() {
        let prepared = super::sanitize_for_composer("<img src=\"//audit.invalid/image\">");
        assert_eq!(prepared.image_sources.len(), 1);
        assert!(
            prepared
                .image_sources
                .values()
                .any(|url| url == "https://audit.invalid/image")
        );
        assert!(!prepared.html.contains("audit.invalid"));
        let saved = super::sanitize_composition(&prepared.html, &prepared.image_sources);
        assert!(saved.html.contains("https://audit.invalid/image"));
    }
    use super::*;

    #[test]
    fn retained_images_roundtrip_without_restoring_deleted_text_or_images() {
        let source = r#"<p>Delete this sentence.</p><p>Keep this sentence.</p><img src="https://a.invalid/one"><img src="https://a.invalid/two">"#;
        let prepared = sanitize_for_composer(source);
        assert_eq!(prepared.image_sources.len(), 2);
        assert!(!prepared.html.contains("https://a.invalid"));
        let first = prepared
            .image_sources
            .iter()
            .find(|(_, url)| url.ends_with("one"))
            .unwrap()
            .0;
        let edited = prepared
            .html
            .replace("<p>Delete this sentence.</p>", "")
            .replace(&format!("<img src=\"{first}\">"), "");
        let sent = sanitize_composition(&edited, &prepared.image_sources).html;
        assert!(!sent.contains("Delete this sentence"));
        assert!(!sent.contains("https://a.invalid/one"));
        assert!(sent.contains("https://a.invalid/two"));
        assert!(sent.contains("Keep this sentence"));
        assert!(!sent.contains("wind-image"));
    }

    #[test]
    fn image_metadata_never_authorizes_a_local_body_or_unsafe_url() {
        let images = BTreeMap::from([("placeholder".into(), "javascript:alert(1)".into())]);
        let sent = sanitize_composition(
            r#"<img src="placeholder"><div data-wind-transfert="1/42/INBOX">edited</div>"#,
            &images,
        )
        .html;
        assert!(!sent.contains("javascript"));
        assert!(!sent.contains("data-wind-transfert"));
        assert!(sent.contains("edited"));
    }

    #[test]
    fn rejects_escaped_loads_functions_and_positioning() {
        for style in [
            r"background:u\72l(https://audit.invalid/pixel)",
            r"color:u\72l(https://audit.invalid/pixel)",
            "position:fixed;inset:0;z-index:99",
            "--image:url(https://audit.invalid/pixel);color:var(--image)",
            "color:rgb(var(--red),0,0)",
            "font-size:100000px;padding:999999em;line-height:9000",
        ] {
            let html = format!("<p style=\"{style}\">text</p>");
            let clean = sanitize_for_composer(&html).html;
            for forbidden in [
                "audit.invalid",
                "fixed",
                "inset",
                "z-index",
                "var(",
                "100000",
                "999999",
                "9000",
            ] {
                assert!(!clean.contains(forbidden), "{style}: {clean}");
            }
        }
    }

    #[test]
    fn blocks_relative_images_and_bounds_legacy_layout() {
        let clean = sanitize_for_composer(
            r#"<img src="/pixel" width="100000"><table cellpadding="99999"><tr><td>ok</td></tr></table>"#,
        );
        assert!(!clean.html.contains("/pixel"));
        assert!(!clean.html.contains("100000"));
        assert!(!clean.html.contains("99999"));
        assert_eq!(clean.remote_images_blocked, 1);
    }

    #[test]
    fn preserves_ordinary_formatting_after_rejected_nested_function() {
        let clean = sanitize_for_composer(r#"<p style="COLOR:red;background-color:color-mix(in srgb,rgb(0,0,0),white);padding:4px 8px;font-size:14px">ok</p>"#).html;
        assert!(clean.contains("color:red"), "{clean}");
        assert!(clean.contains("padding:4px 8px"), "{clean}");
        assert!(clean.contains("font-size:14px"), "{clean}");
        assert!(!clean.contains("color-mix"), "{clean}");
    }

    #[test]
    fn retains_image_only_markup_without_private_attributes() {
        let clean = sanitize_for_composer(
            r#"<img src="data:image/png;base64,AA==" data-wind-transfert="1/2/INBOX">"#,
        )
        .html;
        assert!(clean.contains("data:image/png;base64,AA=="));
        assert!(!clean.contains("data-wind"));
    }
}
