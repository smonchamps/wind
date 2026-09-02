//! Secure rendering of HTML emails — defence in depth validated in Phase 0
//! (PHASE0.md §1, html-render spike):
//!
//! 1. `ammonia` removes scripts, event handlers and dangerous URLs;
//! 2. remote images are replaced by a neutral pixel (privacy: no tracking
//!    pixel, no IP address leak);
//! 3. [`email_document`] produces the document to display in a `sandbox`
//!    iframe: its CSP `default-src 'none'` guarantees that even a bypass
//!    of layers 1-2 can neither execute nor exfiltrate.
//!
//! Assumed limit (documented by a test): the textual CSS filtering can be
//! bypassed by escaping — layer 3 is the one that counts. A real CSS parser
//! (`lightningcss`) will come for the fidelity of `<style>` blocks.

mod sanitize;

pub use sanitize::{BLOCKED_PIXEL, ImagePolicy, Sanitized, sanitize, sanitize_with};

/// A message body reduced to its text — the raw material of a quote
/// (reply, forward — Phase 2).
///
/// Sanitizing FIRST: `ammonia` removes scripts and styles with their
/// content, so that no code can disguise itself as quoted text. The
/// conversion is delegated to `mail-parser` (Phase 0 decision), which only
/// breaks on `<p>` and `<br>` (measured): a pre-pass therefore turns block
/// ends into `<br>` — a display heuristic on already sanitized HTML, not
/// security parsing.
pub fn body_text(html: &str) -> String {
    let sanitized = sanitize(html);
    let text = mail_parser::decoders::html::html_to_text(&block_ends_to_breaks(&sanitized.html));
    collapse_blank_lines(text.trim())
}

/// Block ends → line breaks, table cells → spaces.
/// `ammonia` emits lowercase tags: the case is already normalized.
fn block_ends_to_breaks(html: &str) -> String {
    const BLOCK_ENDS: [&str; 12] = [
        "</div>",
        "</tr>",
        "</li>",
        "</blockquote>",
        "</h1>",
        "</h2>",
        "</h3>",
        "</h4>",
        "</h5>",
        "</h6>",
        "</table>",
        "</ul>",
    ];
    let mut result = html.replace("</td>", "</td> ").replace("</th>", "</th> ");
    for tag in BLOCK_ENDS {
        result = result.replace(tag, &format!("{tag}<br>"));
    }
    result
}

/// Never more than one blank line in a row: nested blocks produce bursts
/// of breaks that are worthless in a quote.
fn collapse_blank_lines(text: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_blank = false;
    for line in text.lines() {
        let line = line.trim_end();
        let blank = line.is_empty();
        if blank && previous_blank {
            continue;
        }
        lines.push(line);
        previous_blank = blank;
    }
    lines.join("\n")
}

/// The palette baked into the document: the ink and the background of the
/// active theme, passed by the shell (review A42 — 14 dark themes made the
/// white slab of the body reachable by any user on a dark OS). The
/// document is self-contained: the sandbox iframe never sees the host's
/// CSS tokens, so the values are written here verbatim.
pub struct Palette {
    ink: String,
    bg: String,
}

impl Palette {
    /// A hue only enters as `#rrggbb`: anything else falls back to the
    /// default — the value is written inside a `<style>`, never free text
    /// in the document.
    pub fn new(ink: Option<&str>, bg: Option<&str>) -> Self {
        Self {
            ink: safe_hue(ink, "#222222"),
            bg: safe_hue(bg, "#ffffff"),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::new(None, None)
    }
}

fn safe_hue(hue: Option<&str>, default: &str) -> String {
    match hue {
        Some(t)
            if t.len() == 7
                && t.starts_with('#')
                && t[1..].bytes().all(|o| o.is_ascii_hexdigit()) =>
        {
            t.to_ascii_lowercase()
        }
        _ => default.to_string(),
    }
}

/// Complete document to load in a `sandbox` iframe (through `srcdoc`): the
/// production model is "one CSP per message", embedded in the document
/// itself. The CSP follows the image policy: it only opens `https:` if the
/// user asked for the remote images.
///
/// **Hosting constraint (proven by experiment, 2026-07-12)**: a `srcdoc`
/// document inherits the CSP of the host page, and a CSP can only
/// tighten. The host must therefore allow at least `img-src data: https:`
/// and `style-src 'unsafe-inline'` — THIS document remains the restrictive
/// layer per message (remote images blocked by default, and never
/// cleartext `http:` even when granted).
pub fn email_document(sanitized_html: &str, policy: ImagePolicy, palette: &Palette) -> String {
    // Audit 2026-09-01: granting remote images is granting HTTPS — never
    // cleartext. And `no-referrer`: without it, every remote image received
    // a `Referer` of origin `tauri.localhost`, a "Wind client" signature
    // offered to the tracker.
    let img_sources = match policy {
        ImagePolicy::BlockRemote => "data: cid:",
        ImagePolicy::AllowRemote => "data: cid: https:",
    };
    let Palette { ink, bg } = palette;
    // A44 (PLAN-RETOURS-V3 R4): the document's scrollbars are native,
    // overlaid — the thumb follows `color-scheme`, derived from the baked
    // background. Without it, a -night background keeps a light… dark,
    // invisible thumb: the "unscrollable body" that had opened A7.
    let scheme = scheme_of_bg(bg);
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"referrer\" content=\"no-referrer\">\
         <meta http-equiv=\"Content-Security-Policy\" \
         content=\"default-src 'none'; img-src {img_sources}; style-src 'unsafe-inline'\">\
         <style>:root{{color-scheme:{scheme}}}\
         body{{font-family:system-ui,sans-serif;margin:12px;color:{ink};\
         background:{bg};overflow-wrap:break-word}}</style>\
         </head><body>{sanitized_html}</body></html>"
    )
}

/// `dark` or `light` from the luminance of the background (Rec. 601). The
/// background comes out of [`safe_hue`]: always `#rrggbb` — the fallback
/// covers the impossible without panicking (zero `unwrap` in production).
fn scheme_of_bg(bg: &str) -> &'static str {
    let v = u32::from_str_radix(bg.get(1..).unwrap_or(""), 16).unwrap_or(0xff_ff_ff);
    let (r, g, b) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
    if 299 * r + 587 * g + 114 * b < 128_000 {
        "dark"
    } else {
        "light"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_text_strips_tags_and_decodes_entities() {
        let text = body_text("<p>Hello &amp; welcome&nbsp;!</p><p>See you tomorrow.</p>");
        assert!(text.contains("Hello & welcome"));
        assert!(text.contains("See you tomorrow."));
        assert!(!text.contains('<'));
    }

    #[test]
    fn body_text_separates_block_elements_with_line_breaks() {
        let text = body_text("<div>line 1</div><div>line 2</div>");
        assert_eq!(text.lines().count(), 2, "{text:?}");
    }

    /// Newsletters are table soups: the cells must stay separated, the
    /// rows too.
    #[test]
    fn body_text_keeps_table_structure_readable() {
        let text = body_text(
            "<table><tr><td>left</td><td>right</td></tr><tr><td>bottom</td></tr></table>",
        );
        assert!(text.contains("left right"), "{text:?}");
        assert!(text.lines().count() >= 2, "{text:?}");
    }

    #[test]
    fn body_text_never_stacks_blank_lines() {
        let text = body_text("<div><p>top</p></div><div></div><div><p>bottom</p></div>");
        assert!(!text.contains("\n\n\n"), "{text:?}");
    }

    /// The content of a script must never end up in a quote.
    #[test]
    fn body_text_drops_script_content_entirely() {
        let text = body_text("<p>visible</p><script>alert('hidden')</script>");
        assert!(text.contains("visible"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("hidden"));
    }

    #[test]
    fn email_document_embeds_csp_and_content() {
        let document = email_document(
            "<p>hello</p>",
            ImagePolicy::BlockRemote,
            &Palette::default(),
        );
        assert!(document.contains("default-src 'none'"));
        assert!(document.contains("img-src data: cid:;"));
        assert!(document.contains("<p>hello</p>"));
        assert!(document.contains("<meta name=\"referrer\" content=\"no-referrer\">"));
    }

    /// Audit 2026-09-01: granting remote images is granting HTTPS — never
    /// cleartext (`http:`), and never a `Referer` that signs "Wind client"
    /// to the tracker.
    #[test]
    fn email_document_opens_https_images_only_on_request() {
        let document = email_document("<p>x</p>", ImagePolicy::AllowRemote, &Palette::default());
        assert!(document.contains("img-src data: cid: https:;"));
        assert!(!document.contains("http:"), "{document}");
        assert!(document.contains("default-src 'none'"));
        assert!(document.contains("<meta name=\"referrer\" content=\"no-referrer\">"));
    }

    #[test]
    fn email_document_bakes_the_theme_palette() {
        // Review A42: the body follows the theme — ink and background passed
        // by the shell, written in the <style> of the self-contained document.
        let palette = Palette::new(Some("#EDEFED"), Some("#2b3034"));
        let document = email_document("<p>x</p>", ImagePolicy::BlockRemote, &palette);
        assert!(document.contains("color:#edefed"), "{document}");
        assert!(document.contains("background:#2b3034"), "{document}");
    }

    #[test]
    fn email_document_declares_the_color_scheme_of_the_background() {
        // A44 (PLAN-RETOURS-V3 R4): the iframe document's scrollbars are
        // native, overlaid — their thumb follows `color-scheme`. Without a
        // declaration, the document is in the light scheme and the (dark)
        // thumb disappears on -night backgrounds. The scheme is derived
        // from the BAKED background: dark → dark, light → light.
        let dark = Palette::new(Some("#edefed"), Some("#2b3034"));
        let document = email_document("<p>x</p>", ImagePolicy::BlockRemote, &dark);
        assert!(document.contains("color-scheme:dark"), "{document}");
        let light = Palette::default();
        let document = email_document("<p>x</p>", ImagePolicy::BlockRemote, &light);
        assert!(document.contains("color-scheme:light"), "{document}");
    }

    #[test]
    fn email_document_refuses_a_malformed_hue() {
        // A free value never enters the <style>: outside #rrggbb, back to
        // the default — the document stays inert.
        let palette = Palette::new(Some("red;}</style><script>"), Some("#12345"));
        let document = email_document("<p>x</p>", ImagePolicy::BlockRemote, &palette);
        assert!(document.contains("color:#222222"), "{document}");
        assert!(document.contains("background:#ffffff"), "{document}");
        assert!(!document.contains("script>alert"), "{document}");
    }
}
