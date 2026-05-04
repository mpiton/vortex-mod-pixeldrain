//! Vortex Pixeldrain WASM plugin.
//!
//! Implements the plugin contract used by the Vortex plugin host:
//! - `can_handle(url)` → `"true"` / `"false"`
//! - `supports_playlist(url)` → always `"false"` (single-file hoster)
//! - `extract_links(url)` → JSON metadata for the resolved file
//! - `resolve_stream_url(input)` → direct CDN URL
//!
//! Network access is delegated to the host via `http_request`. JSON
//! parsing is pure (`api_client.rs`) so it can be exercised natively
//! without WASM.

pub mod api_client;
pub mod error;
pub mod url_matcher;

#[cfg(target_family = "wasm")]
mod plugin_api;

use serde::Serialize;

use crate::api_client::FileInfo;
use crate::error::PluginError;
use crate::url_matcher::UrlKind;

// ── IPC DTOs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExtractLinksResponse {
    pub kind: &'static str,
    pub files: Vec<FileLink>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FileLink {
    pub id: String,
    pub url: String,
    pub filename: Option<String>,
    pub size_bytes: Option<u64>,
    pub direct_url: String,
    pub resumable: bool,
}

// ── Routing helpers ──────────────────────────────────────────────────────────

pub fn handle_can_handle(url: &str) -> String {
    bool_to_string(matches!(url_matcher::classify_url(url), UrlKind::File))
}

pub fn handle_supports_playlist(_url: &str) -> String {
    bool_to_string(false)
}

fn bool_to_string(b: bool) -> String {
    if b {
        "true".into()
    } else {
        "false".into()
    }
}

pub fn ensure_file_url(url: &str) -> Result<(), PluginError> {
    match url_matcher::classify_url(url) {
        UrlKind::File => Ok(()),
        UrlKind::Unknown => Err(PluginError::UnsupportedUrl(url.to_string())),
    }
}

// ── Response builders ────────────────────────────────────────────────────────

pub fn build_extract_links_response(source_url: &str, info: FileInfo) -> ExtractLinksResponse {
    let id = info.id.clone();
    let direct_url = api_client::direct_download_url(&id);
    let link = FileLink {
        id,
        url: source_url.to_string(),
        filename: Some(info.name),
        size_bytes: Some(info.size),
        direct_url,
        resumable: true,
    };
    ExtractLinksResponse {
        kind: "file",
        files: vec![link],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info() -> FileInfo {
        FileInfo {
            id: "abc123".into(),
            name: "archive.zip".into(),
            size: 2048,
            mime_type: Some("application/zip".into()),
        }
    }

    // ── Routing ─────────────────────────────────────────────────────────────

    #[test]
    fn can_handle_recognises_file_url() {
        assert_eq!(handle_can_handle("https://pixeldrain.com/u/abc123"), "true");
    }

    #[test]
    fn can_handle_rejects_unrelated() {
        assert_eq!(handle_can_handle("https://example.com/u/abc123"), "false");
    }

    #[test]
    fn supports_playlist_always_false() {
        assert_eq!(
            handle_supports_playlist("https://pixeldrain.com/u/abc123"),
            "false"
        );
    }

    #[test]
    fn ensure_file_url_accepts_file() {
        ensure_file_url("https://pixeldrain.com/u/abc123").unwrap();
    }

    #[test]
    fn ensure_file_url_rejects_unrelated() {
        let err = ensure_file_url("https://example.com/u/abc123").unwrap_err();
        assert!(matches!(err, PluginError::UnsupportedUrl(_)));
    }

    // ── Response builder ────────────────────────────────────────────────────

    #[test]
    fn build_extract_links_response_includes_metadata() {
        let r = build_extract_links_response("https://pixeldrain.com/u/abc123", sample_info());
        assert_eq!(r.kind, "file");
        assert_eq!(r.files.len(), 1);
        let f = &r.files[0];
        assert_eq!(f.id, "abc123");
        assert_eq!(f.url, "https://pixeldrain.com/u/abc123");
        assert_eq!(f.filename.as_deref(), Some("archive.zip"));
        assert_eq!(f.size_bytes, Some(2048));
        assert_eq!(f.direct_url, "https://pixeldrain.com/api/file/abc123");
        assert!(f.resumable);
    }

    #[test]
    fn extract_links_response_serialises_kind_file() {
        let r = build_extract_links_response("https://pixeldrain.com/u/abc123", sample_info());
        let json = serde_json::to_string(&r).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["kind"], "file");
        assert!(parsed["files"][0]["resumable"].as_bool().unwrap_or(false));
    }
}
