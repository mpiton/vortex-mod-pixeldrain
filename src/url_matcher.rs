//! Pixeldrain URL detection and parsing.
//!
//! Pixeldrain only exposes a single resource shape that the plugin
//! cares about:
//!
//! ```text
//! https://pixeldrain.com/u/<id>
//! ```
//!
//! The `<id>` is an alphanumeric token of 6+ characters. List shapes
//! (`/l/<id>`, multi-file albums) are intentionally out of scope: the
//! plugin only resolves single-file hoster links. Anything else falls
//! through to [`UrlKind::Unknown`].
//!
//! Allowed hosts: `pixeldrain.com`, `www.pixeldrain.com`. Other shapes
//! (api endpoints, embedded players) are rejected.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlKind {
    /// Single file link: `pixeldrain.com/u/<id>`
    File,
    /// Anything else.
    Unknown,
}

pub fn classify_url(url: &str) -> UrlKind {
    let Some(path) = pixeldrain_path(url) else {
        return UrlKind::Unknown;
    };
    if file_regex().is_match(path) {
        UrlKind::File
    } else {
        UrlKind::Unknown
    }
}

/// Extract the file id (`<id>`) from a recognised file URL.
pub fn extract_file_id(url: &str) -> Option<String> {
    let path = pixeldrain_path(url)?;
    file_regex()
        .captures(path)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

/// Returns the normalised path of an http(s) Pixeldrain URL, or `None`
/// if the URL is non-Pixeldrain / non-http(s) / malformed.
fn pixeldrain_path(url: &str) -> Option<&str> {
    let (host, path) = validate_and_split(url)?;
    if !is_pixeldrain_host(host) {
        return None;
    }
    Some(normalize_path(path))
}

fn is_pixeldrain_host(host: &str) -> bool {
    ["pixeldrain.com", "www.pixeldrain.com"]
        .iter()
        .any(|h| host.eq_ignore_ascii_case(h))
}

fn normalize_path(path: &str) -> &str {
    let no_frag = path.split('#').next().unwrap_or("");
    let no_query = no_frag.split('?').next().unwrap_or("");
    no_query.trim_end_matches('/')
}

fn file_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"^/u/([A-Za-z0-9]{6,})$")
            .expect("file_regex: compile-time constant regex must compile")
    })
}

fn validate_and_split(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let (authority, path_and_query) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let authority_no_user = authority.rsplit('@').next().unwrap_or(authority);
    let host = extract_host(authority_no_user)?;
    if host.is_empty() {
        return None;
    }
    Some((host, path_and_query))
}

/// Extract the host portion (without port) from an authority string.
fn extract_host(authority: &str) -> Option<&str> {
    if authority.is_empty() {
        return None;
    }
    if let Some(rest) = authority.strip_prefix('[') {
        let close = rest.find(']')?;
        return Some(&authority[..=close + 1]);
    }
    let host = authority.split(':').next().unwrap_or(authority);
    (!host.is_empty()).then_some(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://pixeldrain.com/u/abc123", UrlKind::File)]
    #[case("https://www.pixeldrain.com/u/abc123def", UrlKind::File)]
    #[case("http://pixeldrain.com/u/abc123", UrlKind::File)]
    #[case("https://pixeldrain.com/u/Abc123XYZ/", UrlKind::File)]
    #[case("https://pixeldrain.com/l/listid", UrlKind::Unknown)]
    #[case("https://pixeldrain.com/api/file/abc123", UrlKind::Unknown)]
    #[case("https://pixeldrain.com/", UrlKind::Unknown)]
    #[case("https://example.com/u/abc123", UrlKind::Unknown)]
    #[case("ftp://pixeldrain.com/u/abc123", UrlKind::Unknown)]
    #[case("not a url", UrlKind::Unknown)]
    fn classify_url_recognises_shapes(#[case] url: &str, #[case] expected: UrlKind) {
        assert_eq!(classify_url(url), expected);
    }

    #[test]
    fn classify_handles_query_and_fragment() {
        assert_eq!(
            classify_url("https://pixeldrain.com/u/abc123?foo=bar#x"),
            UrlKind::File
        );
    }

    #[test]
    fn classify_rejects_short_id() {
        assert_eq!(
            classify_url("https://pixeldrain.com/u/abc"),
            UrlKind::Unknown,
            "Pixeldrain ids are 6+ alphanumerics; shorter shapes must not match"
        );
    }

    #[test]
    fn classify_rejects_non_alphanumeric_id() {
        assert_eq!(
            classify_url("https://pixeldrain.com/u/abc-123"),
            UrlKind::Unknown
        );
    }

    #[test]
    fn extract_file_id_from_simple_path() {
        assert_eq!(
            extract_file_id("https://pixeldrain.com/u/abc123"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_file_id_strips_trailing_slash() {
        assert_eq!(
            extract_file_id("https://pixeldrain.com/u/abc123/"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_file_id_handles_query() {
        assert_eq!(
            extract_file_id("https://pixeldrain.com/u/abc123?download"),
            Some("abc123".into())
        );
    }

    #[test]
    fn extract_file_id_other_host_returns_none() {
        assert_eq!(extract_file_id("https://example.com/u/abc123"), None);
    }

    #[test]
    fn extract_file_id_list_shape_returns_none() {
        assert_eq!(extract_file_id("https://pixeldrain.com/l/abc123"), None);
    }
}
