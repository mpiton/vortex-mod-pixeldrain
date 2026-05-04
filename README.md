# vortex-mod-pixeldrain

Pixeldrain WASM plugin for [Vortex](https://github.com/mpiton/vortex). Resolves
public Pixeldrain `https://pixeldrain.com/u/<id>` links to their direct download
URL via the public JSON API.

## Features

- Single JSON API call (`/api/file/{id}/info`) — no HTML scraping, no captcha,
  no wait timer
- Direct download URL is `https://pixeldrain.com/api/file/{id}` — same host,
  resume-friendly (Pixeldrain serves binary content with `Accept-Ranges: bytes`)
- Maps Pixeldrain's two error envelope shapes onto the Vortex plugin error
  vocabulary:
  - `{"success": false, "value": "file_not_found"}` → `PluginError::Offline`
  - any other `{"success": false, "value": "..."}` → `PluginError::ApiError`
- Forward-compatible `FileInfo` parser — extra fields returned by the API
  (views, downloads, hashes, ads flags…) are ignored without rejecting the
  response

## URL shapes recognised

- `https://pixeldrain.com/u/<id>`
- `https://www.pixeldrain.com/u/<id>`
- `https://pixeldrain.com/u/<id>/`
- `https://pixeldrain.com/u/<id>?download`

`<id>` is an alphanumeric token of 6+ characters. List shapes
(`pixeldrain.com/l/...`) are **not** in scope here — multi-file albums are a
crawler concern and live in a separate plugin.

## Plugin contract

The plugin exports the standard Vortex plugin contract:

| Function                 | Input        | Output                        |
|--------------------------|--------------|-------------------------------|
| `can_handle`             | URL string   | `"true"` / `"false"`          |
| `supports_playlist`      | URL string   | always `"false"`              |
| `extract_links`          | URL string   | JSON `ExtractLinksResponse`   |
| `resolve_stream_url`     | JSON `{url}` | direct API URL string         |

`ExtractLinksResponse` mirrors the `LinkStatus::Online` shape used by the
host's link-check pipeline: each `files[]` entry carries `filename`,
`size_bytes`, `direct_url`, and `resumable: true`.

## Build

```bash
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
```

Resulting WASM: `target/wasm32-wasip1/release/vortex_mod_pixeldrain.wasm`.

## Install (development)

```bash
PLUGIN_DIR="$HOME/.local/share/dev.vortex.app/plugins/vortex-mod-pixeldrain"
mkdir -p "$PLUGIN_DIR"
cp plugin.toml "$PLUGIN_DIR/plugin.toml"
cp target/wasm32-wasip1/release/vortex_mod_pixeldrain.wasm \
   "$PLUGIN_DIR/vortex-mod-pixeldrain.wasm"
```

Vortex picks up the new plugin via the file watcher; no restart needed.

## Tests

```bash
cargo test                              # 55 native + WASM smoke tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The JSON API fixtures live in `tests/fixtures/*.json` — nine variants covering
simple zip, large file, unicode filename, missing mime type, `success: false`
envelope (`file_not_found` and `rate_limit`), and a zero-size payload.

## License

GPL-3.0 — same as Vortex core.
