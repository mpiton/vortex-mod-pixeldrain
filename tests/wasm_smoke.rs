//! Smoke test: load the compiled `.wasm` via Extism and call the pure
//! `can_handle` / `supports_playlist` exports.
//!
//! `extract_links` and `resolve_stream_url` need a real `http_request`
//! round-trip — exercised by the host's own integration tests, not
//! here. The stub `http_request` returns an HTTP-like JSON envelope so
//! the WASM module loads without unresolved imports.
//!
//! Skipped unless the WASM artifact is present at
//! `target/wasm32-wasip1/release/vortex_mod_pixeldrain.wasm`. To produce
//! it:
//!
//! ```bash
//! cargo build --target wasm32-wasip1 --release
//! ```

use std::path::PathBuf;

use extism::{Function, UserData, Val, PTR};

const WASM_REL_PATH: &str = "target/wasm32-wasip1/release/vortex_mod_pixeldrain.wasm";

fn wasm_path() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(WASM_REL_PATH);
    p.exists().then_some(p)
}

fn stub_http_request() -> Function {
    Function::new(
        "http_request",
        [PTR],
        [PTR],
        UserData::<()>::default(),
        |plugin, _inputs, outputs, _user_data: UserData<()>| {
            let body = r#"{"status":200,"headers":{},"body":"{\"id\":\"abc123\",\"name\":\"foo.zip\",\"size\":42}"}"#;
            let handle = plugin.memory_new(body)?;
            outputs[0] = Val::I64(handle.offset() as i64);
            Ok(())
        },
    )
}

fn load_plugin(path: &PathBuf) -> extism::Plugin {
    let manifest = extism::Manifest::new([extism::Wasm::file(path)]);
    extism::Plugin::new(&manifest, [stub_http_request()], true).expect("load wasm")
}

/// Resolve the WASM artefact path or skip the calling test with a build hint.
macro_rules! require_wasm {
    () => {
        match wasm_path() {
            Some(p) => p,
            None => {
                eprintln!(
                    "skipping: build with `cargo build --target wasm32-wasip1 --release` first"
                );
                return;
            }
        }
    };
}

#[test]
fn wasm_can_handle_recognises_pixeldrain_file_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("can_handle", "https://pixeldrain.com/u/abc123def")
        .expect("can_handle call");
    assert_eq!(result.trim(), "true");
}

#[test]
fn wasm_can_handle_rejects_unrelated_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("can_handle", "https://example.com/some/page")
        .expect("can_handle call");
    assert_eq!(result.trim(), "false");
}

#[test]
fn wasm_supports_playlist_always_false() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("supports_playlist", "https://pixeldrain.com/u/abc123def")
        .expect("supports_playlist call");
    assert_eq!(result.trim(), "false");
}

#[test]
fn wasm_extract_links_returns_online_metadata() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let result: String = plugin
        .call("extract_links", "https://pixeldrain.com/u/abc123def")
        .expect("extract_links call");
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("response is JSON");
    assert_eq!(parsed["kind"], "file");
    assert_eq!(parsed["files"][0]["filename"], "foo.zip");
    assert_eq!(parsed["files"][0]["size_bytes"], 42);
    assert_eq!(
        parsed["files"][0]["direct_url"],
        "https://pixeldrain.com/api/file/abc123"
    );
    assert_eq!(parsed["files"][0]["resumable"], true);
}

#[test]
fn wasm_resolve_stream_url_returns_direct_api_url() {
    let path = require_wasm!();
    let mut plugin = load_plugin(&path);
    let input = r#"{"url":"https://pixeldrain.com/u/abc123def"}"#;
    let result: String = plugin
        .call("resolve_stream_url", input)
        .expect("resolve_stream_url call");
    assert_eq!(
        result.trim(),
        "https://pixeldrain.com/api/file/abc123def",
        "direct URL must hit /api/file/<id>, no extra hop"
    );
}
