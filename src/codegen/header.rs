//! Centralized state-hash marker helper.
//!
//! Every generated file begins with a marker pointing at the state file it
//! was produced from, plus that file's content hash. The user app's
//! `build.rs` re-reads this header, recomputes the hash, and panics on
//! mismatch — making stale codegen a hard build failure.
//!
//! Format (byte-stable, no timestamps, no clocks):
//!
//! ```text
//! // AUTO-GENERATED from <state-relative-path> @ <hash>
//! //
//! // Do not edit by hand. Run `blast gen all` after mutating state.
//! ```
//!
//! The trailing blank line is included so callers can prepend the marker
//! directly to their generated body.

use std::path::{Path, PathBuf};

use crate::{
    error::{BlastError, BlastResult},
    state::content_hash,
};

const STATE_DIR_RELATIVE: &str = "storage/blast/state";
const APP_STATE_FILE: &str = "app.ron";
const RESOURCES_SUBDIR: &str = "resources";
const SCHEMA_RELATIVE: &str = "src/database/schema.rs";

const MARKER_PREFIX: &str = "// AUTO-GENERATED from ";
const MARKER_SEPARATOR: &str = " @ ";
const MARKER_BLANK_LINE: &str = "//";
const MARKER_FOOTER: &str = "// Do not edit by hand. Run `blast gen all` after mutating state.";

/// Format a marker header given an already-resolved relative path + hash.
///
/// The relative path is emitted verbatim — callers must normalize separators
/// to forward slashes for cross-platform stability before calling this fn.
pub fn marker(state_relative_path: &str, content_hash: &str) -> String {
    format!(
        "{prefix}{path}{sep}{hash}\n{blank}\n{footer}\n\n",
        prefix = MARKER_PREFIX,
        path = state_relative_path,
        sep = MARKER_SEPARATOR,
        hash = content_hash,
        blank = MARKER_BLANK_LINE,
        footer = MARKER_FOOTER,
    )
}

/// Convenience: compute the relative path + content hash of a state file
/// and return the formatted marker header.
///
/// `state_path` must live under `project_root` (typically
/// `<project_root>/storage/blast/state/...`); the relative form is what
/// gets embedded so the user app's `build.rs` can resolve it portably.
pub fn marker_for_state_file(project_root: &Path, state_path: &Path) -> BlastResult<String> {
    let relative = state_path.strip_prefix(project_root)?;
    let relative_str = match relative.to_str() {
        Some(s) => s.replace('\\', "/"),
        None => {
            return Err(BlastError::Invalid(format!("non-utf8 state path: {}", relative.display())));
        }
    };
    let hash = content_hash(state_path)?;
    Ok(marker(&relative_str, &hash))
}

/// Absolute path to the per-resource state file under the conventional
/// `storage/blast/state/resources/<table>.ron` layout.
pub fn resource_state_path(project_root: &Path, table: &str) -> PathBuf {
    project_root.join(STATE_DIR_RELATIVE).join(RESOURCES_SUBDIR).join(format!("{}.ron", table))
}

/// Absolute path to the app-wide state file at
/// `storage/blast/state/app.ron`.
pub fn app_state_path(project_root: &Path) -> PathBuf {
    project_root.join(STATE_DIR_RELATIVE).join(APP_STATE_FILE)
}

/// Convenience: marker for a resource state file by table name. Reads
/// the on-disk state file at the conventional path and embeds its hash.
pub fn marker_for_resource(project_root: &Path, table: &str) -> BlastResult<String> {
    let state_path = resource_state_path(project_root, table);
    marker_for_state_file(project_root, &state_path)
}

/// Convenience: marker for the app-wide state file.
pub fn marker_for_app(project_root: &Path) -> BlastResult<String> {
    let state_path = app_state_path(project_root);
    marker_for_state_file(project_root, &state_path)
}

/// Convenience: marker for the diesel schema file. Used by structs/models
/// codegen which is schema-driven, not state-driven — but the same stale
/// detection mechanism applies (regenerate when schema.rs changes).
pub fn marker_for_schema(project_root: &Path) -> BlastResult<String> {
    let schema_path = project_root.join(SCHEMA_RELATIVE);
    marker_for_state_file(project_root, &schema_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_marker(file_contents: &str) -> Option<(String, String)> {
        let first_line = file_contents.lines().next()?;
        let rest = first_line.strip_prefix(MARKER_PREFIX)?;
        let sep_idx = rest.find(MARKER_SEPARATOR)?;
        let path = rest[..sep_idx].trim();
        let hash = rest[sep_idx + MARKER_SEPARATOR.len()..].trim();
        if path.is_empty() || hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some((path.to_string(), hash.to_string()))
    }

    #[test]
    fn marker_formats_expected_shape() {
        let header = marker("storage/blast/state/resources/users.ron", "abc123");
        let expected = "// AUTO-GENERATED from storage/blast/state/resources/users.ron @ abc123\n//\n// Do not edit by hand. Run `blast gen all` after mutating state.\n\n";
        assert_eq!(header, expected);
    }

    const FAKE_BLAKE3_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn marker_round_trips_through_parse() {
        let path = "storage/blast/state/resources/orders.ron";
        let header = marker(path, FAKE_BLAKE3_HEX);
        let parsed = parse_marker(&header);
        match parsed {
            Some((p, h)) => {
                assert_eq!(p, path);
                assert_eq!(h, FAKE_BLAKE3_HEX);
            }
            None => panic!("expected marker to parse"),
        }
    }

    #[test]
    fn parse_marker_accepts_marker_followed_by_body() {
        let body = format!("{header}export const FOO = 1\n", header = marker("storage/blast/state/app.ron", FAKE_BLAKE3_HEX));
        match parse_marker(&body) {
            Some((p, h)) => {
                assert_eq!(p, "storage/blast/state/app.ron");
                assert_eq!(h, FAKE_BLAKE3_HEX);
            }
            None => panic!("expected marker to parse from prefixed body"),
        }
    }

    #[test]
    fn parse_marker_returns_none_on_short_hash() {
        let body = "// AUTO-GENERATED from foo.ron @ deadbeef\n";
        assert!(parse_marker(body).is_none(), "63-char-or-less hash must not parse — BLAKE3 hex is exactly 64");
    }

    #[test]
    fn parse_marker_returns_none_on_long_hash() {
        let oversized = format!("{FAKE_BLAKE3_HEX}ff");
        let body = format!("// AUTO-GENERATED from foo.ron @ {oversized}\n");
        assert!(parse_marker(&body).is_none(), "65+-char hash must not parse");
    }

    #[test]
    fn parse_marker_returns_none_on_non_hex_hash() {
        let bad = "g".repeat(64);
        let body = format!("// AUTO-GENERATED from foo.ron @ {bad}\n");
        assert!(parse_marker(&body).is_none(), "non-hex chars must reject even at 64-char length");
    }

    #[test]
    fn parse_marker_returns_none_on_missing_header() {
        let body = "// some other comment\nfn main() {}\n";
        assert!(parse_marker(body).is_none());
    }

    #[test]
    fn parse_marker_returns_none_on_empty_input() {
        assert!(parse_marker("").is_none());
    }

    #[test]
    fn parse_marker_returns_none_when_separator_missing() {
        let body = "// AUTO-GENERATED from no-separator-here\n";
        assert!(parse_marker(body).is_none());
    }

    #[test]
    fn parse_marker_returns_none_when_hash_missing() {
        let body = "// AUTO-GENERATED from foo.ron @ \n";
        assert!(parse_marker(body).is_none());
    }

    #[test]
    fn parse_marker_returns_none_when_path_missing() {
        let body = "// AUTO-GENERATED from  @ abc123\n";
        assert!(parse_marker(body).is_none());
    }

    #[test]
    fn marker_is_byte_stable() {
        let a = marker("x.ron", "h");
        let b = marker("x.ron", "h");
        assert_eq!(a, b);
    }
}
