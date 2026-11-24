use std::sync::LazyLock;

use regex::Regex;
use url::Url;

/// Known email spy-pixel, open-tracking, and behavioral analytics domains.
/// Matched by exact host or subdomain suffix (e.g., `t.sidekickopen80.com`).
const TRACKER_DOMAIN_SUFFIXES: &[&str] = &[
    "list-manage.com",
    "mailchimp.com",
    "mandrillapp.com",
    "sendgrid.net",
    "hubspot.com",
    "hubspotemail.net",
    "hubspotlinks.com",
    "sidekickopen.com",
    "mixmax.com",
    "superhuman.com",
    "mailtrack.io",
    "yesware.com",
    "bananatag.com",
    "boomeranggmail.com",
    "salesloft.com",
    "outreach.io",
    "getnotify.com",
    "readnotify.com",
    "streak.com",
    "mailfoogae.appspot.com",
    "postmarkapp.com",
    "pstmrk.it",
    "klaviyomail.com",
    "trk.klaviyomail.com",
    "braze.com",
    "appboy.com",
    "customer.io",
    "customeriomail.com",
    "iterable.com",
    "mktotracking.com",
    "marketo.com",
    "pardot.com",
    "exacttarget.com",
    "salesforce.com",
    "constantcontact.com",
    "r20.rs6.net",
    "intercom-mail.com",
    "intercom.io",
    "dripemail2.com",
    "convertkit-mail.com",
    "convertkit-mail2.com",
    "beehiiv.com",
    "litmus.com",
    "emltrk.com",
    "returnpath.net",
    "250ok.com",
    "google-analytics.com",
    "doubleclick.net",
    "facebook.com",
    "connect.facebook.net",
    "px.ads.linkedin.com",
    "bat.bing.com",
    "t.co",
    "click.pstmrk.it",
    "open.substack.com",
    "email.mg.",
    "mailgun.org",
    "sparkpostmail.com",
    "awstrack.me",
];

static TRACKING_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(/track/open|/wf/open|/email/open|/pixel|/beacon|/o\.gif|/t\.gif|/1x1|/spacer\.gif|/open\.php|/imp\?|/collect\?|/tr\?|[?&](utm_source|mc_eid|hsa_|trk=|open_id=|pixel=))",
    )
    .expect("valid tracking path regex")
});

static HIDDEN_STYLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(display\s*:\s*none|visibility\s*:\s*hidden|opacity\s*:\s*0(\.0+)?\b|width\s*:\s*[01](px)?\b|height\s*:\s*[01](px)?\b)",
    )
    .expect("valid hidden style regex")
});

static CSS_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)url\(\s*['"]?(https?://[^)'"\s]+)['"]?\s*\)"#)
        .expect("valid css url regex")
});

static CSS_IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)@import\s+(?:url\(\s*)?['"]?(https?://[^)'"\s;]+)['"]?\s*\)?[^;]*;?"#)
        .expect("valid css import regex")
});

#[derive(Debug, Clone)]
pub struct TrackerVerdict {
    pub domain: String,
    pub category: &'static str,
    pub reason: String,
}

/// Extracts host domain from an arbitrary URL string.
pub fn extract_domain(raw_url: &str) -> String {
    Url::parse(raw_url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Checks if a URL belongs to a known tracking domain or matches a spy-pixel endpoint.
pub fn classify_url_tracker(raw_url: &str) -> Option<TrackerVerdict> {
    let parsed = Url::parse(raw_url.trim()).ok()?;
    let host = parsed.host_str()?.to_lowercase();

    // Known domain suffix check (including HubSpot's sidekickopenXX.com wildcard)
    if host.contains("sidekickopen") || host.contains("mailtrack") || host.contains("open-pixel") {
        return Some(TrackerVerdict {
            domain: host,
            category: "tracker_domain",
            reason: "Matched known email spy-pixel domain signature".to_string(),
        });
    }

    for suffix in TRACKER_DOMAIN_SUFFIXES {
        if host == *suffix || host.ends_with(&format!(".{suffix}")) {
            // Avoid false positives on general brand homepage links unless on an image/beacon
            return Some(TrackerVerdict {
                domain: host,
                category: "tracker_domain",
                reason: format!("Blocked known email tracker infrastructure ({suffix})"),
            });
        }
    }

    // Heuristic path/query beacon check
    let path_and_query = match parsed.query() {
        Some(q) => format!("{}?{}", parsed.path(), q),
        None => parsed.path().to_string(),
    };
    if TRACKING_PATH_RE.is_match(&path_and_query) {
        return Some(TrackerVerdict {
            domain: host,
            category: "spy_pixel",
            reason: "URL matches open-tracking pixel or telemetry beacon pattern".to_string(),
        });
    }

    None
}

/// Determines whether an `<img>` element's dimensions or inline CSS indicate a hidden 1x1 spy pixel.
pub fn is_pixel_dimensions(
    width_attr: Option<&str>,
    height_attr: Option<&str>,
    style_attr: Option<&str>,
) -> Option<String> {
    let parse_dim = |v: &str| -> Option<u32> {
        let trimmed = v.trim().trim_end_matches("px").trim();
        trimmed.parse::<u32>().ok()
    };

    if let Some(w) = width_attr.and_then(parse_dim) {
        if w <= 1 {
            return Some(format!("Hidden tracking pixel dimension (width={w}px)"));
        }
    }
    if let Some(h) = height_attr.and_then(parse_dim) {
        if h <= 1 {
            return Some(format!("Hidden tracking pixel dimension (height={h}px)"));
        }
    }
    if let Some(style) = style_attr {
        if HIDDEN_STYLE_RE.is_match(style) {
            return Some("Hidden CSS dimensions/visibility used as tracking pixel".to_string());
        }
    }
    None
}

/// Strips remote `url(http...)` and `@import` beacons from CSS content, returning
/// the sanitized CSS plus every stripped remote beacon URL.
pub fn strip_css_beacons(css: &str) -> (String, Vec<String>) {
    let mut stripped_urls = Vec::new();

    for cap in CSS_IMPORT_RE.captures_iter(css) {
        if let Some(m) = cap.get(1) {
            stripped_urls.push(m.as_str().to_string());
        }
    }
    let without_imports = CSS_IMPORT_RE.replace_all(css, "/* [cipherpost: @import stripped] */");

    for cap in CSS_URL_RE.captures_iter(&without_imports) {
        if let Some(m) = cap.get(1) {
            stripped_urls.push(m.as_str().to_string());
        }
    }
    let sanitized = CSS_URL_RE.replace_all(&without_imports, "none /* [cipherpost: css beacon stripped] */");

    (sanitized.into_owned(), stripped_urls)
}
