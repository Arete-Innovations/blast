#!/bin/bash

# One-shot iteration loop: test, then install. Bails on first failure.
# Warnings silenced (-A warnings); cargo's own progress muted (--quiet).
# Run this AFTER each edit. No need for separate `cargo check` / `cargo test` runs.

set -e

cd "$(dirname "$0")"

export RUSTFLAGS="-A warnings"

cargo test --quiet
cargo install --quiet --path . --root "$HOME/.local" --force
