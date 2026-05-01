#!/bin/bash

# One-shot iteration loop: test, refresh canonical's generated files, then install.
# Bails on first failure. Warnings silenced (-A warnings); cargo progress muted (--quiet).
# Run this AFTER each edit. No need for separate `cargo check` / `cargo test` runs.
#
# The `gen all` step here keeps canonical's baked generated files in sync with
# its current state. Without it, hash drift in `templates/canonical/src/.../generated/`
# silently propagates into every `blast new` and detonates user `cargo build`.

set -e

cd "$(dirname "$0")"

export RUSTFLAGS="-A warnings"

cargo test --quiet

# Refresh canonical's generated files using the just-tested debug binary.
# `cwd` must be canonical for blast to operate on its state files.
(
  cd templates/canonical
  cargo run --quiet --manifest-path ../../Cargo.toml -- gen all > /dev/null

  # Smoke: canonical must compile against its current generated tree on
  # both targets — host (SSR + bin) and wasm32 (browser bundle).
  # Catches the kind of break that only surfaces inside a scaffolded user
  # app (stale TableRow refs, cata_log gating, hash drift in markers, etc).
  echo "==> canonical: cargo check (host)"
  cargo check --quiet
  echo "==> canonical: cargo check --lib --target wasm32-unknown-unknown"
  cargo check --quiet --lib --target wasm32-unknown-unknown
)

cargo install --quiet --path . --root "$HOME/.local" --force
