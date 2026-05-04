//! Fixture-driven integration tests for the Pixeldrain JSON API parser.
//!
//! Each fixture in `tests/fixtures/*.json` mirrors a shape returned by
//! the real `https://pixeldrain.com/api/file/{id}/info` endpoint. The
//! tests validate that:
//!
//! 1. Successful payloads decode into [`FileInfo`] with the correct
//!    fields (id, name, size, mime_type).
//! 2. Error envelopes (`success: false`) are mapped to the expected
//!    [`PluginError`] variant — `Offline` for `file_not_found`, and
//!    `ApiError` for any other `value`.

use std::fs;
use std::path::Path;

use rstest::rstest;
use vortex_mod_pixeldrain::api_client::{parse_file_info, FileInfo};
use vortex_mod_pixeldrain::error::PluginError;

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn load_fixture(name: &str) -> String {
    let path = Path::new(FIXTURES_DIR).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[rstest]
#[case(
    "01_simple_zip.json",
    "abc123def456",
    "archive.zip",
    1_572_864,
    Some("application/zip")
)]
#[case(
    "02_filename_with_spaces.json",
    "xyz9876",
    "My Document v2.pdf",
    843_213,
    Some("application/pdf")
)]
#[case(
    "03_large_file.json",
    "biG12345",
    "ubuntu-server.iso",
    4_509_715_661,
    Some("application/x-iso9660-image")
)]
#[case("04_minimal_payload.json", "tinyId", "notes.txt", 512, None)]
#[case("05_no_mime_type.json", "noMimeId", "binary.dat", 4_096, None)]
#[case(
    "06_unicode_filename.json",
    "uniCodeI",
    "résumé_clé_2026.pdf",
    65_536,
    Some("application/pdf")
)]
#[case("09_zero_size.json", "zeroSize", "empty.txt", 0, Some("text/plain"))]
fn parses_recognised_fixture(
    #[case] fixture: &str,
    #[case] expected_id: &str,
    #[case] expected_name: &str,
    #[case] expected_size: u64,
    #[case] expected_mime: Option<&str>,
) {
    let body = load_fixture(fixture);
    let info: FileInfo = parse_file_info(&body).expect("fixture must parse");
    assert_eq!(info.id, expected_id, "fixture: {fixture}");
    assert_eq!(info.name, expected_name, "fixture: {fixture}");
    assert_eq!(info.size, expected_size, "fixture: {fixture}");
    assert_eq!(
        info.mime_type.as_deref(),
        expected_mime,
        "fixture: {fixture}"
    );
}

#[test]
fn offline_fixture_maps_to_offline_error() {
    let body = load_fixture("07_offline_not_found.json");
    let err = parse_file_info(&body).unwrap_err();
    assert!(
        matches!(err, PluginError::Offline(_)),
        "removed-file responses must surface PluginError::Offline, got: {err:?}"
    );
}

#[test]
fn rate_limit_fixture_maps_to_api_error() {
    let body = load_fixture("08_rate_limited.json");
    let err = parse_file_info(&body).unwrap_err();
    assert!(
        matches!(err, PluginError::ApiError(_)),
        "non-not-found api errors must surface PluginError::ApiError, got: {err:?}"
    );
}

#[test]
fn fixture_count_covers_main_shapes() {
    let entries = fs::read_dir(FIXTURES_DIR)
        .expect("fixtures dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count();
    assert!(
        entries >= 8,
        "expected at least 8 JSON fixtures (success + error shapes), found {entries}"
    );
}
