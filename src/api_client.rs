//! Pixeldrain JSON API wrapper.
//!
//! The plugin host marshals each network call as a JSON-encoded
//! [`HttpRequest`] / [`HttpResponse`] pair through the `http_request`
//! host function. The pure parsing in this module makes it testable
//! natively without touching the host.
//!
//! Pixeldrain endpoints used:
//! - `GET https://pixeldrain.com/api/file/{id}/info` → metadata JSON
//! - `GET https://pixeldrain.com/api/file/{id}` → direct binary download

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

const USER_AGENT: &str = "Mozilla/5.0 (Vortex/1.0; +https://vortex-app.com) PixeldrainPlugin/1.0";
const API_BASE: &str = "https://pixeldrain.com/api/file";

// ── HTTP envelope ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
}

/// Reject responses larger than this so a malicious server can't make
/// every JSON parse a multi-megabyte buffer. Real `/info` payloads are
/// well under 4 KB, so 256 KB is a generous ceiling.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

impl HttpResponse {
    pub fn into_success_body(self) -> Result<String, PluginError> {
        if (200..300).contains(&self.status) {
            if self.body.len() > MAX_BODY_BYTES {
                return Err(PluginError::HttpStatus {
                    status: self.status,
                    message: format!("body exceeds {MAX_BODY_BYTES} bytes"),
                });
            }
            Ok(self.body)
        } else if self.status == 404 || self.status == 410 {
            Err(PluginError::Offline(format!("status {}", self.status)))
        } else {
            Err(PluginError::HttpStatus {
                status: self.status,
                message: truncate(&self.body, 256),
            })
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut cut = max;
        while !s.is_char_boundary(cut) && cut > 0 {
            cut -= 1;
        }
        format!("{}…", &s[..cut])
    }
}

pub fn parse_http_response(raw: &str) -> Result<HttpResponse, PluginError> {
    serde_json::from_str(raw).map_err(|e| PluginError::HostResponse(e.to_string()))
}

// ── Request builders ─────────────────────────────────────────────────────────

pub fn build_info_request(file_id: &str) -> Result<String, PluginError> {
    let url = format!("{API_BASE}/{file_id}/info");
    serialise_get(&url)
}

fn serialise_get(url: &str) -> Result<String, PluginError> {
    let mut headers = HashMap::new();
    headers.insert("User-Agent".to_string(), USER_AGENT.to_string());
    headers.insert("Accept".to_string(), "application/json".to_string());
    let req = HttpRequest {
        method: "GET".into(),
        url: url.to_string(),
        headers,
        body: None,
    };
    serde_json::to_string(&req).map_err(PluginError::SerdeJson)
}

/// Build the direct download URL exposed to the Vortex download engine.
///
/// Pixeldrain serves the binary from the same `/api/file/{id}` endpoint
/// that returns metadata when `/info` is appended, so no separate CDN
/// hop is required.
pub fn direct_download_url(file_id: &str) -> String {
    format!("{API_BASE}/{file_id}")
}

// ── File metadata ────────────────────────────────────────────────────────────

/// Subset of Pixeldrain's `/api/file/{id}/info` payload that the plugin
/// actually uses. Extra fields (views, ads, hashes, etc.) are ignored
/// via `serde(default)` to stay forward-compatible.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// Pixeldrain returns a `{success: false, ...}` JSON envelope for
/// recoverable errors instead of an HTTP error status — those still
/// need to map onto `PluginError::Offline` / `ApiError`.
#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Parse a `/info` response body into [`FileInfo`].
///
/// Recognises the API's two failure shapes:
/// - `{"success": false, "value": "file_not_found", ...}` →
///   [`PluginError::Offline`] (treat removed/missing files as offline)
/// - `{"success": false, "value": "...", "message": "..."}` →
///   [`PluginError::ApiError`] for any other API rejection
pub fn parse_file_info(body: &str) -> Result<FileInfo, PluginError> {
    if let Ok(envelope) = serde_json::from_str::<ApiErrorEnvelope>(body) {
        if envelope.success == Some(false) {
            let value = envelope.value.unwrap_or_default();
            let message = envelope.message.unwrap_or_default();
            return Err(classify_api_error(&value, &message));
        }
    }
    let info: FileInfo = serde_json::from_str(body)?;
    if info.id.is_empty() || info.name.is_empty() {
        return Err(PluginError::ApiError(
            "Pixeldrain /info response missing id or name".into(),
        ));
    }
    Ok(info)
}

fn classify_api_error(value: &str, message: &str) -> PluginError {
    let detail = if message.is_empty() {
        value.to_string()
    } else {
        format!("{value}: {message}")
    };
    match value {
        "file_not_found" | "not_found" => PluginError::Offline(detail),
        _ => PluginError::ApiError(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HTTP envelope ───────────────────────────────────────────────────────

    #[test]
    fn parse_http_response_round_trips_success() {
        let raw = r#"{"status":200,"headers":{},"body":"{}"}"#;
        let resp = parse_http_response(raw).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "{}");
    }

    #[test]
    fn into_success_body_passes_2xx() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: "{}".into(),
        };
        assert_eq!(resp.into_success_body().unwrap(), "{}");
    }

    #[test]
    fn into_success_body_maps_404_to_offline() {
        let resp = HttpResponse {
            status: 404,
            headers: HashMap::new(),
            body: "".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::Offline(_)));
    }

    #[test]
    fn into_success_body_maps_410_to_offline() {
        let resp = HttpResponse {
            status: 410,
            headers: HashMap::new(),
            body: "".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::Offline(_)));
    }

    #[test]
    fn into_success_body_rejects_oversized_2xx_payload() {
        let resp = HttpResponse {
            status: 200,
            headers: HashMap::new(),
            body: "x".repeat(MAX_BODY_BYTES + 1),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::HttpStatus { status: 200, .. }));
    }

    #[test]
    fn into_success_body_maps_500_to_http_status() {
        let resp = HttpResponse {
            status: 500,
            headers: HashMap::new(),
            body: "boom".into(),
        };
        let err = resp.into_success_body().unwrap_err();
        assert!(matches!(err, PluginError::HttpStatus { status: 500, .. }));
    }

    // ── Request builders ────────────────────────────────────────────────────

    #[test]
    fn build_info_request_targets_api_endpoint() {
        let json = build_info_request("abc123").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["method"], "GET");
        assert_eq!(v["url"], "https://pixeldrain.com/api/file/abc123/info");
        assert_eq!(
            v["headers"]["Accept"], "application/json",
            "JSON API requires an explicit Accept header"
        );
        assert!(
            v["headers"]["User-Agent"].is_string(),
            "request must carry a User-Agent so Pixeldrain doesn't reject the fetch"
        );
    }

    #[test]
    fn direct_download_url_is_stable_format() {
        assert_eq!(
            direct_download_url("abc123"),
            "https://pixeldrain.com/api/file/abc123"
        );
    }

    // ── File info parser ────────────────────────────────────────────────────

    #[test]
    fn parse_file_info_extracts_required_fields() {
        let body = r#"{
            "id": "abc123",
            "name": "myfile.zip",
            "size": 1572864,
            "mime_type": "application/zip",
            "views": 0,
            "downloads": 0
        }"#;
        let info = parse_file_info(body).unwrap();
        assert_eq!(info.id, "abc123");
        assert_eq!(info.name, "myfile.zip");
        assert_eq!(info.size, 1_572_864);
        assert_eq!(info.mime_type.as_deref(), Some("application/zip"));
    }

    #[test]
    fn parse_file_info_accepts_minimal_payload() {
        let body = r#"{"id":"x","name":"y","size":1}"#;
        let info = parse_file_info(body).unwrap();
        assert_eq!(info.id, "x");
        assert_eq!(info.name, "y");
        assert_eq!(info.size, 1);
        assert!(info.mime_type.is_none());
    }

    #[test]
    fn parse_file_info_maps_file_not_found_to_offline() {
        let body = r#"{"success": false, "value": "file_not_found", "message": "File not found"}"#;
        let err = parse_file_info(body).unwrap_err();
        assert!(matches!(err, PluginError::Offline(_)));
    }

    #[test]
    fn parse_file_info_maps_other_api_error_to_api_error() {
        let body = r#"{"success": false, "value": "rate_limit", "message": "slow down"}"#;
        let err = parse_file_info(body).unwrap_err();
        assert!(matches!(err, PluginError::ApiError(_)));
    }

    #[test]
    fn parse_file_info_rejects_garbage_json() {
        let err = parse_file_info("not json").unwrap_err();
        assert!(matches!(err, PluginError::SerdeJson(_)));
    }

    #[test]
    fn parse_file_info_rejects_missing_id() {
        let body = r#"{"id":"","name":"x","size":1}"#;
        let err = parse_file_info(body).unwrap_err();
        assert!(matches!(err, PluginError::ApiError(_)));
    }

    #[test]
    fn parse_file_info_rejects_missing_name() {
        let body = r#"{"id":"x","name":"","size":1}"#;
        let err = parse_file_info(body).unwrap_err();
        assert!(matches!(err, PluginError::ApiError(_)));
    }
}
