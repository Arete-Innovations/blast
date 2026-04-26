//! Schema-version upgraders for RON state files.
//!
//! State files carry `schema_version: u32` so Blast can migrate older
//! files forward without breaking on disk. Two flavors of upgrader live
//! here:
//!
//! - **Typed app upgraders** (`AppUpgrader`): operate on a fully
//!   deserialized `AppState`. Suitable when v(N) → v(N+1) only adds
//!   optional fields covered by `#[serde(default)]`.
//! - **Raw-text resource upgraders** (`ResourceRawUpgrader`): operate on
//!   the raw RON string before deserialization. Required when a step
//!   reshapes a field type (e.g. `BTreeSet` → `BTreeMap`) so the v(N)
//!   bytes can no longer round-trip through the v(N+1) types.
//!
//! Resource upgraders run inside the `load_resource` IO entry point —
//! see the `state::io` module — producing the bumped raw text that is
//! then deserialized as `ResourceState`. The typed `upgrade_resource`
//! entry point exists for tests and is a no-op once the typed
//! `schema_version` already equals `RESOURCE_SCHEMA_VERSION`.

use crate::error::{BlastError, BlastResult};
use crate::state::app::{
    AppPolicySection, AppState, APP_SCHEMA_VERSION, ICONS_SECTION_KEY, THEME_SECTION_KEY,
};
use crate::state::icons::IconConfig;
use crate::state::resource::{ResourceState, RESOURCE_SCHEMA_VERSION};
use crate::state::theme::ThemeConfig;
use regex::Regex;

type AppUpgrader = fn(&mut AppState) -> BlastResult<()>;
type ResourceRawUpgrader = fn(&str) -> BlastResult<String>;

const APP_UPGRADERS: &[(u32, AppUpgrader)] = &[
    (1, upgrade_app_v1_to_v2),
    (2, upgrade_app_v2_to_v3),
    (3, upgrade_app_v3_to_v4),
];

/// Raw-text upgraders, indexed by `from_version`. Each entry takes the
/// RON text at `from_version` and returns the text at `from_version+1`,
/// including a bumped `schema_version` field.
const RESOURCE_RAW_UPGRADERS: &[(u32, ResourceRawUpgrader)] =
    &[(1, upgrade_resource_v1_to_v2)];

/// v1 → v2: purely additive. No fields were added to `AppState` between v1
/// and v2 that require migration — the bump just advances the version token.
fn upgrade_app_v1_to_v2(state: &mut AppState) -> BlastResult<()> {
    state.schema_version = 2;
    Ok(())
}

/// v2 → v3: purely additive. Adds optional `nav` (NavConfig) and `pages`
/// ([Page]) sections to `AppState`. Both default to absent, so existing
/// v2 files load cleanly with no nav or pages sections.
fn upgrade_app_v2_to_v3(state: &mut AppState) -> BlastResult<()> {
    state.schema_version = 3;
    Ok(())
}

/// v3 → v4: additive. Inject `theme` and `icons` sections with their
/// `Default` content if absent. Existing keys are preserved untouched —
/// users may already have customized one or both sections by hand.
fn upgrade_app_v3_to_v4(state: &mut AppState) -> BlastResult<()> {
    state.schema_version = 4;
    if !state.sections.contains_key(THEME_SECTION_KEY) {
        state.sections.insert(
            THEME_SECTION_KEY.to_string(),
            AppPolicySection::Theme(ThemeConfig::default()),
        );
    }
    if !state.sections.contains_key(ICONS_SECTION_KEY) {
        state.sections.insert(
            ICONS_SECTION_KEY.to_string(),
            AppPolicySection::Icons(IconConfig::default()),
        );
    }
    Ok(())
}

pub fn upgrade_app(state: &mut AppState) -> BlastResult<()> {
    while state.schema_version < APP_SCHEMA_VERSION {
        let from = state.schema_version;
        let entry = APP_UPGRADERS.iter().find(|(v, _)| *v == from);
        let upgrader = match entry {
            Some((_, f)) => f,
            None => {
                return Err(BlastError::Invalid(format!(
                    "no app upgrader registered for schema_version={from}"
                )))
            }
        };
        upgrader(state)?;
        if state.schema_version <= from {
            return Err(BlastError::Invalid(format!(
                "app upgrader for v{from} did not bump schema_version"
            )));
        }
    }
    if state.schema_version > APP_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "app schema_version={} newer than supported {}",
            state.schema_version, APP_SCHEMA_VERSION
        )));
    }
    Ok(())
}

/// Sanity-check entry point that runs after the file is already
/// deserialized. Resource shape changes that break deserialize MUST be
/// done at the raw-text layer in `upgrade_resource_raw`.
pub fn upgrade_resource(state: &mut ResourceState) -> BlastResult<()> {
    if state.schema_version > RESOURCE_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "resource schema_version={} newer than supported {}",
            state.schema_version, RESOURCE_SCHEMA_VERSION
        )));
    }
    if state.schema_version < RESOURCE_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "resource schema_version={} not migrated; load_resource() should have \
             upgraded the raw RON before deserialize",
            state.schema_version,
        )));
    }
    Ok(())
}

/// Walk raw RON text through every registered raw upgrader until it
/// reaches `RESOURCE_SCHEMA_VERSION`. Returns the upgraded text,
/// suitable for `ron::from_str::<ResourceState>`.
pub fn upgrade_resource_raw(raw: &str) -> BlastResult<String> {
    let mut current = raw.to_string();
    let mut version = parse_resource_schema_version(&current)?;

    while version < RESOURCE_SCHEMA_VERSION {
        let entry = RESOURCE_RAW_UPGRADERS.iter().find(|(v, _)| *v == version);
        let upgrader = match entry {
            Some((_, f)) => f,
            None => {
                return Err(BlastError::Invalid(format!(
                    "no raw resource upgrader registered for schema_version={version}"
                )))
            }
        };
        let next = upgrader(&current)?;
        let next_version = parse_resource_schema_version(&next)?;
        if next_version <= version {
            return Err(BlastError::Invalid(format!(
                "raw resource upgrader for v{version} did not bump schema_version (still {next_version})"
            )));
        }
        current = next;
        version = next_version;
    }

    if version > RESOURCE_SCHEMA_VERSION {
        return Err(BlastError::Invalid(format!(
            "resource schema_version={version} newer than supported {RESOURCE_SCHEMA_VERSION}"
        )));
    }

    Ok(current)
}

/// Pull `schema_version: <u32>` out of a RON text body.
fn parse_resource_schema_version(raw: &str) -> BlastResult<u32> {
    let re = Regex::new(r"\bschema_version\s*:\s*(\d+)").map_err(BlastError::from)?;
    let captures = match re.captures(raw) {
        Some(c) => c,
        None => {
            return Err(BlastError::Invalid(
                "resource RON missing schema_version field".to_string(),
            ))
        }
    };
    let raw_num = match captures.get(1) {
        Some(m) => m.as_str(),
        None => {
            return Err(BlastError::Invalid(
                "schema_version regex matched but capture group 1 absent".to_string(),
            ))
        }
    };
    raw_num.parse::<u32>().map_err(|err| {
        BlastError::Invalid(format!("schema_version not a u32: {raw_num}: {err}"))
    })
}

/// v1 → v2: reshape `filterable_columns` from `BTreeSet<FieldName>` (a
/// RON sequence) to `BTreeMap<FieldName, FilterKind>` (a RON map),
/// defaulting every column to `FilterKind::Eq`. The rest of the v1 RON
/// is forward-compatible thanks to `#[serde(default)]` on the new
/// fields (`singular_override`, `soft_delete`, `relations`).
fn upgrade_resource_v1_to_v2(raw: &str) -> BlastResult<String> {
    let mut bumped = bump_schema_version(raw, 1, 2)?;

    // Match: filterable_columns: [ ... ]   (allowing nested whitespace/newlines)
    // The contents can include quoted strings ("email", "title", ...).
    let re = Regex::new(r"(?s)filterable_columns\s*:\s*\[([^\]]*)\]")
        .map_err(BlastError::from)?;

    bumped = re.replace_all(&bumped, |caps: &regex::Captures| {
        let inner = &caps[1];
        let entries: Vec<String> = inner
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|col| format!("{col}: Eq"))
            .collect();
        if entries.is_empty() {
            "filterable_columns: {}".to_string()
        } else {
            format!("filterable_columns: {{{}}}", entries.join(", "))
        }
    }).into_owned();

    Ok(bumped)
}

/// Replace exactly one `schema_version: <from>` token with
/// `schema_version: <to>`. Errors if the token is missing or the value
/// does not match `from`.
fn bump_schema_version(raw: &str, from: u32, to: u32) -> BlastResult<String> {
    let re = Regex::new(r"(\bschema_version\s*:\s*)(\d+)").map_err(BlastError::from)?;
    let mut found = false;
    let mut mismatch: Option<String> = None;
    let bumped = re.replace(raw, |caps: &regex::Captures| {
        found = true;
        let prefix = &caps[1];
        let actual = &caps[2];
        let parsed = match actual.parse::<u32>() {
            Ok(n) => n,
            Err(_parse_err) => {
                mismatch = Some(actual.to_string());
                return caps[0].to_string();
            }
        };
        if parsed != from {
            mismatch = Some(actual.to_string());
            return caps[0].to_string();
        }
        format!("{prefix}{to}")
    });
    if !found {
        return Err(BlastError::Invalid(
            "raw RON missing schema_version for upgrader bump".to_string(),
        ));
    }
    match mismatch {
        Some(actual) => Err(BlastError::Invalid(format!(
            "schema_version mismatch in upgrader: expected {from}, found {actual}"
        ))),
        None => Ok(bumped.into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_extracts_decimal() {
        let raw = "ResourceState(\n  schema_version: 1,\n  name: \"users\",\n)";
        assert_eq!(parse_resource_schema_version(raw).expect("parse"), 1);
    }

    #[test]
    fn parse_version_errors_when_missing() {
        let raw = "ResourceState(\n  name: \"users\",\n)";
        let err = parse_resource_schema_version(raw).unwrap_err();
        match err {
            BlastError::Invalid(msg) => assert!(msg.contains("schema_version")),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn bump_replaces_only_when_value_matches() {
        let raw = "ResourceState(schema_version: 1, name: \"x\")";
        let bumped = bump_schema_version(raw, 1, 2).expect("bump");
        assert!(bumped.contains("schema_version: 2"));
        assert!(!bumped.contains("schema_version: 1"));
    }

    #[test]
    fn bump_rejects_value_mismatch() {
        let raw = "ResourceState(schema_version: 7, name: \"x\")";
        let err = bump_schema_version(raw, 1, 2).unwrap_err();
        match err {
            BlastError::Invalid(msg) => assert!(msg.contains("mismatch")),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn upgrade_v1_to_v2_reshapes_filterable_columns() {
        let raw = r#"ResourceState(
  schema_version: 1,
  filterable_columns: ["email", "title"],
)"#;
        let upgraded = upgrade_resource_v1_to_v2(raw).expect("upgrade");
        assert!(upgraded.contains("schema_version: 2"));
        assert!(upgraded.contains("filterable_columns: {\"email\": Eq, \"title\": Eq}"));
    }

    #[test]
    fn upgrade_v1_to_v2_handles_empty_filterable() {
        let raw = r#"ResourceState(
  schema_version: 1,
  filterable_columns: [],
)"#;
        let upgraded = upgrade_resource_v1_to_v2(raw).expect("upgrade empty");
        assert!(upgraded.contains("filterable_columns: {}"));
    }

    #[test]
    fn upgrade_v1_to_v2_preserves_unrelated_lines() {
        let raw = r#"ResourceState(
  schema_version: 1,
  name: "users",
  ws_events: None,
)"#;
        let upgraded = upgrade_resource_v1_to_v2(raw).expect("upgrade");
        assert!(upgraded.contains("name: \"users\""));
        assert!(upgraded.contains("ws_events: None"));
    }

    #[test]
    fn upgrade_resource_raw_is_idempotent_on_v2() {
        let raw = r#"ResourceState(
  schema_version: 2,
  filterable_columns: {"email": Eq},
)"#;
        let upgraded = upgrade_resource_raw(raw).expect("no-op");
        assert_eq!(upgraded, raw);
    }

    #[test]
    fn upgrade_resource_raw_rejects_future_version() {
        let raw = r#"ResourceState(schema_version: 99)"#;
        let err = upgrade_resource_raw(raw).unwrap_err();
        match err {
            BlastError::Invalid(msg) => assert!(msg.contains("newer than supported")),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }
}
