pub mod trackers;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use lol_html::{element, rewrite_str, text, RewriteStrSettings};

use crate::error::{CipherError, CipherResult};
use crate::models::BlockedTracker;
use crate::sanitizer::trackers::{
    classify_url_tracker, extract_domain, is_pixel_dimensions, strip_css_beacons,
};

/// Result of running an email's raw HTML through CipherPost's Active Privacy & Tracker Shield.
#[derive(Debug, Clone)]
pub struct SanitizedEmailHtml {
    /// Strict default output: scripts, trackers, CSS beacons stripped AND remote images blocked.
    pub html_blocked: String,
    /// Opt-in output ("Load Remote Assets"): scripts, trackers, CSS beacons STILL stripped,
    /// but legitimate non-tracker remote images are permitted.
    pub html_with_assets: String,
    /// Deduplicated structured list of all trackers/threats neutralized.
    pub blocked_trackers: Vec<BlockedTracker>,
    /// Number of non-tracker remote images held back until user enables "Load Remote Assets".
    pub remote_assets_count: usize,
}

const DANGEROUS_ATTR_PREFIXES: &[&str] = &["on"];
const DANGEROUS_URI_SCHEMES: &[&str] = &["javascript:", "vbscript:", "data:text/html"];

/// Sanitizes untrusted email HTML strictly inside Rust before anything touches the WebView.
///
/// Guarantees:
/// 1. Strips `<script>`, `<iframe>`, `<object>`, `<embed>`, `<applet>`, `<form>`, `<base>`,
///    `<meta http-equiv="refresh">`, inline `on*` event handlers, and `javascript:` links.
/// 2. Detects and completely removes 1x1/hidden tracking pixels and known spy-pixel domains.
/// 3. Strips remote CSS beacons (`url(http...)`, `@import`, `<link rel="stylesheet">`).
/// 4. Blocks all remaining remote `<img>` sources by default while producing a safe
///    `html_with_assets` variant for the user's one-click "Load Remote Assets" toggle.
pub fn sanitize_email_html(raw_html: &str) -> CipherResult<SanitizedEmailHtml> {
    let trackers_acc: Rc<RefCell<Vec<BlockedTracker>>> = Rc::new(RefCell::new(Vec::new()));
    let remote_assets_acc: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));

    let html_blocked = run_pipeline(
        raw_html,
        true,
        Rc::clone(&trackers_acc),
        Rc::clone(&remote_assets_acc),
    )?;

    // Second pass produces the opt-in remote-assets HTML (trackers & scripts STILL removed!)
    let dummy_trackers = Rc::new(RefCell::new(Vec::new()));
    let dummy_assets = Rc::new(RefCell::new(0usize));
    let html_with_assets = run_pipeline(raw_html, false, dummy_trackers, dummy_assets)?;

    // Deduplicate blocked trackers by (url, category)
    let mut seen = HashSet::new();
    let mut blocked_trackers = Vec::new();
    for item in trackers_acc.borrow().iter() {
        let key = format!("{}::{}", item.url, item.category);
        if seen.insert(key) {
            blocked_trackers.push(item.clone());
        }
    }

    let remote_assets_count = *remote_assets_acc.borrow();

    Ok(SanitizedEmailHtml {
        html_blocked,
        html_with_assets,
        blocked_trackers,
        remote_assets_count,
    })
}

fn is_remote_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("//")
}

fn run_pipeline(
    raw_html: &str,
    block_remote_images: bool,
    trackers: Rc<RefCell<Vec<BlockedTracker>>>,
    remote_assets_count: Rc<RefCell<usize>>,
) -> CipherResult<String> {
    let t_scripts = Rc::clone(&trackers);
    let t_links = Rc::clone(&trackers);
    let t_imgs = Rc::clone(&trackers);
    let t_attrs = Rc::clone(&trackers);
    let t_style_text = Rc::clone(&trackers);
    let assets_counter = Rc::clone(&remote_assets_count);

    let sanitized = rewrite_str(
        raw_html,
        RewriteStrSettings {
            element_content_handlers: vec![
                // 1. Strip active executable / exfiltration tags completely
                element!(
                    "script, noscript, iframe, frame, frameset, object, embed, applet, form, base, meta[http-equiv]",
                    move |el| {
                        let tag = el.tag_name();
                        if tag != "noscript" {
                            t_scripts.borrow_mut().push(BlockedTracker {
                                url: format!("<{tag}>"),
                                domain: "inline-html".to_string(),
                                category: "script_injection".to_string(),
                                reason: format!("Stripped active/dangerous <{tag}> element"),
                                element: tag,
                            });
                        }
                        el.remove();
                        Ok(())
                    }
                ),
                // 2. Strip remote stylesheets & prefetch/preconnect beacons (<link>)
                element!("link", move |el| {
                    if let Some(href) = el.get_attribute("href") {
                        if is_remote_url(&href) {
                            t_links.borrow_mut().push(BlockedTracker {
                                domain: extract_domain(&href),
                                url: href,
                                category: "remote_stylesheet".to_string(),
                                reason: "Stripped remote <link> resource / CSS beacon".to_string(),
                                element: "link".to_string(),
                            });
                        }
                    }
                    el.remove();
                    Ok(())
                }),
                // 3. Strip remote CSS beacons inside <style> blocks
                text!("style", move |t| {
                    let chunk = t.as_str();
                    if chunk.contains("http://") || chunk.contains("https://") || chunk.to_ascii_lowercase().contains("@import") {
                        let (cleaned, urls) = strip_css_beacons(chunk);
                        for url in urls {
                            t_style_text.borrow_mut().push(BlockedTracker {
                                domain: extract_domain(&url),
                                url,
                                category: "css_beacon".to_string(),
                                reason: "Stripped remote CSS url()/@import tracking beacon".to_string(),
                                element: "style".to_string(),
                            });
                        }
                        t.set_str(cleaned);
                    }
                    Ok(())
                }),
                // 4. Inspect every <img> for 1x1 spy pixels, tracker domains, and remote image blocking
                element!("img", move |el| {
                    let src = el.get_attribute("src").unwrap_or_default();
                    let width = el.get_attribute("width");
                    let height = el.get_attribute("height");
                    let style = el.get_attribute("style");

                    if is_remote_url(&src) {
                        // First check 1x1 / hidden pixel dimensions
                        if let Some(pixel_reason) = is_pixel_dimensions(
                            width.as_deref(),
                            height.as_deref(),
                            style.as_deref(),
                        ) {
                            t_imgs.borrow_mut().push(BlockedTracker {
                                domain: extract_domain(&src),
                                url: src,
                                category: "spy_pixel".to_string(),
                                reason: pixel_reason,
                                element: "img".to_string(),
                            });
                            el.remove();
                            return Ok(());
                        }

                        // Next check known spy-pixel domains & beacon URL heuristics
                        if let Some(verdict) = classify_url_tracker(&src) {
                            t_imgs.borrow_mut().push(BlockedTracker {
                                domain: verdict.domain,
                                url: src,
                                category: verdict.category.to_string(),
                                reason: verdict.reason,
                                element: "img".to_string(),
                            });
                            el.remove();
                            return Ok(());
                        }

                        // Legitimate external image: block by default unless user toggled Load Remote Assets
                        *assets_counter.borrow_mut() += 1;
                        if block_remote_images {
                            let alt = el
                                .get_attribute("alt")
                                .filter(|a| !a.trim().is_empty())
                                .unwrap_or_else(|| extract_domain(&src));
                            el.remove_attribute("src");
                            el.remove_attribute("srcset");
                            let _ = el.set_attribute("data-cipherpost-blocked-src", &src);
                            let _ = el.set_attribute("data-cipherpost-alt", &alt);
                            let _ = el.set_attribute(
                                "style",
                                "display:inline-block;padding:6px 10px;border:1px dashed #2e3440;border-radius:6px;background:#111318;color:#7c8599;font-family:ui-monospace,monospace;font-size:11px;",
                            );
                            el.replace(
                                &format!(
                                    r#"<span class="cp-blocked-asset" data-blocked-domain="{}">[Remote image blocked: {}]</span>"#,
                                    escape_html_attr(&extract_domain(&src)),
                                    escape_html_text(&alt)
                                ),
                                lol_html::html_content::ContentType::Html,
                            );
                        } else {
                            let _ = el.set_attribute("referrerpolicy", "no-referrer");
                            let _ = el.set_attribute("loading", "lazy");
                        }
                    }
                    Ok(())
                }),
                // 5. Strip dangerous attributes (on*, javascript:, inline style CSS beacons, background attrs) on all elements
                element!("*", move |el| {
                    let mut to_remove = Vec::new();
                    let mut rewritten_style: Option<String> = None;
                    let mut strip_href = false;

                    for attr in el.attributes() {
                        let name = attr.name().to_ascii_lowercase();
                        let val = attr.value();

                        // Remove all inline JS event handlers (onclick, onload, onerror, ...)
                        if DANGEROUS_ATTR_PREFIXES.iter().any(|p| name.starts_with(p)) && name.len() > 2 {
                            t_attrs.borrow_mut().push(BlockedTracker {
                                url: format!("attr:{name}"),
                                domain: "inline-html".to_string(),
                                category: "script_injection".to_string(),
                                reason: format!("Removed inline event handler attribute '{name}'"),
                                element: el.tag_name(),
                            });
                            to_remove.push(attr.name());
                            continue;
                        }

                        // Remove remote `background="..."` or `poster="..."` beacons
                        if (name == "background" || name == "poster" || name == "ping") && is_remote_url(&val) {
                            t_attrs.borrow_mut().push(BlockedTracker {
                                domain: extract_domain(&val),
                                url: val.clone(),
                                category: "css_beacon".to_string(),
                                reason: format!("Removed remote '{name}' beacon attribute"),
                                element: el.tag_name(),
                            });
                            to_remove.push(attr.name());
                            continue;
                        }

                        // Sanitize inline `style=""` for remote CSS url() beacons
                        if name == "style" && (val.contains("http://") || val.contains("https://")) {
                            let (cleaned, urls) = strip_css_beacons(&val);
                            for url in urls {
                                t_attrs.borrow_mut().push(BlockedTracker {
                                    domain: extract_domain(&url),
                                    url,
                                    category: "css_beacon".to_string(),
                                    reason: "Stripped remote inline style url() beacon".to_string(),
                                    element: el.tag_name(),
                                });
                            }
                            rewritten_style = Some(cleaned);
                        }

                        // Neutralize javascript: / vbscript: hrefs & src
                        if name == "href" || name == "src" || name == "action" {
                            let lower_val = val.trim().to_ascii_lowercase();
                            if DANGEROUS_URI_SCHEMES.iter().any(|s| lower_val.starts_with(s)) {
                                t_attrs.borrow_mut().push(BlockedTracker {
                                    url: val.clone(),
                                    domain: "inline-html".to_string(),
                                    category: "script_injection".to_string(),
                                    reason: format!("Neutralized dangerous URI scheme in '{name}'"),
                                    element: el.tag_name(),
                                });
                                strip_href = true;
                                to_remove.push(attr.name());
                            }
                        }
                    }

                    for name in to_remove {
                        el.remove_attribute(&name);
                    }
                    if let Some(style) = rewritten_style {
                        let _ = el.set_attribute("style", &style);
                    }
                    if strip_href {
                        let _ = el.set_attribute("href", "#blocked");
                    }

                    // Enforce safe external link navigation
                    if el.tag_name() == "a" {
                        let _ = el.set_attribute("rel", "noopener noreferrer nofollow");
                        let _ = el.set_attribute("target", "_blank");
                    }

                    Ok(())
                }),
            ],
            ..RewriteStrSettings::new()
        },
    )
    .map_err(|e| CipherError::Sanitizer(e.to_string()))?;

    Ok(sanitized)
}

fn escape_html_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(input: &str) -> String {
    escape_html_text(input).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_spy_pixels_and_css_beacons_while_preserving_safe_content() {
        let hostile_html = r#"
            <html>
              <head>
                <link rel="stylesheet" href="https://tracker.hubspot.com/beacon.css">
                <style>
                  body { background-image: url('https://cdn.evil-beacon.net/open?id=42'); color: #fff; }
                </style>
                <script>alert('pwned')</script>
              </head>
              <body onload="steal()">
                <p onclick="exfil()">Hello CipherPost!</p>
                <a href="javascript:alert(1)">Bad Link</a>
                <img src="https://example.org/open.gif" width="1" height="1" />
                <img src="https://r.superhuman.com/px/abc123.png" width="20" height="20" />
                <img src="https://cdn.acme.dev/product-diagram.png" alt="Architecture Diagram" width="600" height="320" />
              </body>
            </html>
        "#;

        let out = sanitize_email_html(hostile_html).expect("sanitize should succeed");

        // Scripts, event handlers, and javascript: links must be completely gone
        assert!(!out.html_blocked.contains("<script>"));
        assert!(!out.html_blocked.contains("alert('pwned')"));
        assert!(!out.html_blocked.contains("onload="));
        assert!(!out.html_blocked.contains("onclick="));
        assert!(!out.html_blocked.contains("javascript:"));

        // Spy pixel and known tracker domain must be stripped in BOTH modes
        assert!(!out.html_blocked.contains("open.gif"));
        assert!(!out.html_with_assets.contains("open.gif"));
        assert!(!out.html_blocked.contains("superhuman.com"));
        assert!(!out.html_with_assets.contains("superhuman.com"));

        // CSS beacon and remote stylesheet must be stripped
        assert!(!out.html_blocked.contains("evil-beacon.net"));
        assert!(!out.html_blocked.contains("hubspot.com"));

        // Legitimate image blocked in default mode, allowed in opt-in mode
        assert!(!out.html_blocked.contains("product-diagram.png"));
        assert!(out.html_blocked.contains("Remote image blocked: Architecture Diagram"));
        assert!(out.html_with_assets.contains("product-diagram.png"));
        assert_eq!(out.remote_assets_count, 1);

        // Trackers recorded accurately (link + style beacon + script + onload + onclick + js href + 1x1 img + superhuman img)
        assert!(out.blocked_trackers.len() >= 6);
        assert!(out.html_blocked.contains("Hello CipherPost!"));
    }
}
