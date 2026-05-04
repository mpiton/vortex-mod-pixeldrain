//! WASM-only entry points: `#[plugin_fn]` exports + `#[host_fn]` imports.

use extism_pdk::*;

use crate::api_client::{
    build_info_request, direct_download_url, parse_file_info, parse_http_response, FileInfo,
};
use crate::error::PluginError;
use crate::url_matcher::extract_file_id;
use crate::{
    build_extract_links_response, ensure_file_url, handle_can_handle, handle_supports_playlist,
};

#[host_fn]
extern "ExtismHost" {
    fn http_request(req: String) -> String;
}

#[plugin_fn]
pub fn can_handle(url: String) -> FnResult<String> {
    Ok(handle_can_handle(&url))
}

#[plugin_fn]
pub fn supports_playlist(url: String) -> FnResult<String> {
    Ok(handle_supports_playlist(&url))
}

#[plugin_fn]
pub fn extract_links(url: String) -> FnResult<String> {
    ensure_file_url(&url).map_err(error_to_fn_error)?;
    let info = fetch_info(&url)?;
    let response = build_extract_links_response(&url, info);
    Ok(serde_json::to_string(&response)?)
}

/// Resolve the direct download URL for a Pixeldrain file.
///
/// Input JSON: `{ "url": "..." }` — extra fields are ignored. Returns
/// the `https://pixeldrain.com/api/file/<id>` URL the host hands to the
/// download engine.
#[plugin_fn]
pub fn resolve_stream_url(input: String) -> FnResult<String> {
    #[derive(serde::Deserialize)]
    struct Input {
        url: String,
    }
    let params: Input =
        serde_json::from_str(&input).map_err(|e| error_to_fn_error(PluginError::SerdeJson(e)))?;
    ensure_file_url(&params.url).map_err(error_to_fn_error)?;
    let id = extract_file_id(&params.url)
        .ok_or_else(|| error_to_fn_error(PluginError::UnsupportedUrl(params.url.clone())))?;
    Ok(direct_download_url(&id))
}

fn fetch_info(url: &str) -> FnResult<FileInfo> {
    let id = extract_file_id(url)
        .ok_or_else(|| error_to_fn_error(PluginError::UnsupportedUrl(url.to_string())))?;
    let req = build_info_request(&id).map_err(error_to_fn_error)?;
    // SAFETY: `http_request` is resolved by the Vortex plugin host at
    // load time (see src-tauri/src/adapters/driven/plugin/host_functions.rs:
    // `make_http_request_function`). Invariants:
    //   1. The host registers `http_request` in the `ExtismHost`
    //      namespace before any `#[plugin_fn]` export is callable.
    //   2. The ABI is `(I64) -> I64`; the `#[host_fn]` macro marshals
    //      `String` in/out through Extism memory handles.
    //   3. The host gates the call on the `http` capability declared in
    //      `plugin.toml`; rejections return an error that `?` surfaces.
    //   4. Inputs/outputs are owned JSON strings — no aliasing concerns.
    let raw = unsafe { http_request(req)? };
    let resp = parse_http_response(&raw).map_err(error_to_fn_error)?;
    let body = resp.into_success_body().map_err(error_to_fn_error)?;
    parse_file_info(&body).map_err(error_to_fn_error)
}

fn error_to_fn_error(err: PluginError) -> WithReturnCode<extism_pdk::Error> {
    extism_pdk::Error::msg(err.to_string()).into()
}
