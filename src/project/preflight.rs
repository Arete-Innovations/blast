//! Pre-flight binary check.
//!
//! Runs as the very FIRST step of `blast new` / `blast init`, before any
//! disk writes or DB I/O. Verifies the host has the binaries the
//! post-scaffold pipeline relies on. On any required-missing, returns an
//! error so scaffold aborts BEFORE creating directories or touching
//! Postgres — the user gets a clear "install X" message and an empty
//! filesystem / database.

use crate::{
    error::{BlastError, BlastResult},
    io::traits::{Sink, SinkExt},
};

/// One binary the scaffold pipeline expects on PATH.
pub struct BinCheck {
    pub name: &'static str,
    pub install_hint: &'static str,
    pub required: bool,
}

/// Canonical pre-flight bins. Hard-fails on missing `cargo`/`psql`/`zellij`.
/// Warns if `git` or `cargo-leptos` is absent (latter required for FE
/// development but not at scaffold time).
const CHECKS: &[BinCheck] = &[
    BinCheck {
        name: "cargo",
        install_hint: "rustup: https://rustup.rs",
        required: true,
    },
    BinCheck {
        name: "psql",
        install_hint: "install PostgreSQL client tools — e.g. `pacman -S postgresql` / `apt install postgresql-client`",
        required: true,
    },
    BinCheck {
        name: "zellij",
        install_hint: "`cargo install zellij` — required for the post-scaffold dashboard auto-launch",
        required: true,
    },
    BinCheck {
        name: "git",
        install_hint: "install git — required to clone the catalyst framework on `blast new`",
        required: true,
    },
    BinCheck {
        name: "cargo-leptos",
        install_hint: "`cargo install cargo-leptos --locked` — required for FE dev/build (not blocking scaffold)",
        required: false,
    },
];

/// Run the full pre-flight check. Reports each found bin and each missing
/// bin via the sink. Returns `Err` on any required-missing.
pub fn run(sink: &mut dyn Sink) -> BlastResult<()> {
    run_with(CHECKS, sink)
}

/// Test-friendly variant taking the check list explicitly.
pub fn run_with(checks: &[BinCheck], sink: &mut dyn Sink) -> BlastResult<()> {
    sink.info("preflight: checking required binaries on PATH");

    let mut missing_required: Vec<&BinCheck> = Vec::new();
    let mut missing_optional: Vec<&BinCheck> = Vec::new();

    for check in checks {
        match which::which(check.name) {
            Ok(path) => {
                sink.success(format!("  found {} -> {}", check.name, path.display()));
            }
            Err(_lookup_err) => {
                if check.required {
                    sink.error(format!("  MISSING {} (required) — {}", check.name, check.install_hint));
                    missing_required.push(check);
                } else {
                    sink.warn(format!("  missing {} (optional) — {}", check.name, check.install_hint));
                    missing_optional.push(check);
                }
            }
        }
    }

    if !missing_required.is_empty() {
        let names: Vec<&str> = missing_required.iter().map(|c| c.name).collect();
        return Err(BlastError::Project(format!("missing required binaries: {}; install via package manager", names.join(", "))));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::null::NullSink;

    #[test]
    fn run_passes_when_all_present() {
        // `cargo` and `rustc` are guaranteed to exist when this test runs
        // (we're being executed under cargo). Use them as a stand-in for
        // any always-present required bin.
        let checks = [
            BinCheck {
                name: "cargo",
                install_hint: "(test)",
                required: true,
            },
            BinCheck {
                name: "rustc",
                install_hint: "(test)",
                required: true,
            },
        ];
        let mut sink = NullSink;
        run_with(&checks, &mut sink).expect("preflight ok");
    }

    #[test]
    fn run_fails_on_missing_required() {
        let checks = [BinCheck {
            name: "definitely-not-a-real-binary-xyzzy",
            install_hint: "(test)",
            required: true,
        }];
        let mut sink = NullSink;
        let err = run_with(&checks, &mut sink).expect_err("must fail");
        let msg = format!("{}", err);
        assert!(msg.contains("missing required binaries"), "msg = {}", msg);
        assert!(msg.contains("definitely-not-a-real-binary-xyzzy"), "msg = {}", msg);
    }

    #[test]
    fn run_tolerates_missing_optional() {
        let checks = [
            BinCheck {
                name: "cargo",
                install_hint: "(test)",
                required: true,
            },
            BinCheck {
                name: "definitely-not-a-real-binary-xyzzy",
                install_hint: "(test)",
                required: false,
            },
        ];
        let mut sink = NullSink;
        run_with(&checks, &mut sink).expect("preflight ok despite missing optional");
    }
}
