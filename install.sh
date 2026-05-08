#!/bin/bash

# Test + install blast.
# blast no longer bundles a template tree (post-flip). Catalyst is the
# framework source of truth — iterate there directly. This script only
# builds + installs the blast binary.

set -e

cd "$(dirname "$0")"

export RUSTFLAGS="-A warnings"

cargo test --quiet

cargo install --quiet --path . --force
