# Contributing to vortex-mod-pixeldrain

Thanks for taking the time to contribute! This crate is a WASM plugin for the
[Vortex download manager](https://github.com/mpiton/vortex). It targets
`wasm32-wasip1` via Extism PDK and is loaded by the Vortex host at runtime.

## How to Contribute

### Reporting Bugs

1. Check if the bug has already been reported in [Issues](https://github.com/mpiton/vortex-mod-pixeldrain/issues)
2. If not, create a new issue using the **Bug Report** template
3. Include the Pixeldrain URL shape (without sensitive parts), the `vortex --version`,
   and the plugin version

### Suggesting Features

1. Check existing [Feature Requests](https://github.com/mpiton/vortex-mod-pixeldrain/issues?q=label%3Aenhancement)
2. Open a new issue using the **Feature Request** template
3. Describe the Pixeldrain URL shape or capability and the use case

### Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/your-feature`)
3. Add a **failing test first** — see existing fixtures in `tests/fixtures/*.json`
4. Implement the change in `src/api_client.rs` / `src/url_matcher.rs` / `src/lib.rs`
5. Run `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`
6. Build the WASM artefact (`cargo build --target wasm32-wasip1 --release`)
   and check `wasm_smoke.rs` still passes
7. Commit using [Conventional Commits](https://www.conventionalcommits.org/)
8. Push to your fork and open a Pull Request

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`, `ci`
Scopes: `api-client`, `url-matcher`, `plugin-api`, `error`, `tests`, `build`

Example: `fix(api-client): treat success:false + value:not_found as Offline`

## Development Setup

```bash
# Prerequisites
rustup target add wasm32-wasip1

# Clone
git clone https://github.com/mpiton/vortex-mod-pixeldrain.git
cd vortex-mod-pixeldrain

# Native unit tests + JSON fixtures + WASM smoke
cargo test

# Lint + format
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Build WASM release artefact (~1.05 MiB)
cargo build --target wasm32-wasip1 --release
# → target/wasm32-wasip1/release/vortex_mod_pixeldrain.wasm
```

## Adding a fixture

The Pixeldrain JSON API may evolve. To add a new variant:

1. Save a representative `/api/file/{id}/info` response to
   `tests/fixtures/NN_<short_name>.json`
2. Add a `#[case]` row to `api_fixtures.rs::parses_recognised_fixture`
   (or to the dedicated error tests for `success: false` envelopes)
3. Run `cargo test` — RED first, then make the parser pass without breaking
   any existing fixture
4. Update `fixture_count_covers_main_shapes` if you removed any

## Security

Pixeldrain returns a `{"success": false, ...}` JSON envelope for recoverable
errors instead of an HTTP error status. The parser in
`api_client.rs::parse_file_info` inspects the envelope before falling back to
the plain `FileInfo` shape — **do not bypass that check**. An attacker
controlling the API response could otherwise smuggle a misleading `id`/`name`
through the host.

For sensitive vulnerability reports, see [SECURITY.md](SECURITY.md).

## Code of Conduct

This project follows the upstream
[Vortex Code of Conduct](https://github.com/mpiton/vortex/blob/main/CODE_OF_CONDUCT.md).
By participating, you agree to uphold it.

## Questions

Open a [Discussion](https://github.com/mpiton/vortex-mod-pixeldrain/discussions)
or file an issue using the **Question** template.
