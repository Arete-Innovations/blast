#!/usr/bin/env bash
# blast-reset.sh — wipe generated codegen output, reinstall blast from local source, regenerate.
#
# Use to start a clean iteration loop:
#   - drop every <layer>/generated/ dir under the target project
#   - drop tests/route_alignment_generated.rs (blast-emitted)
#   - reinstall blast from /home/tragdate/codumeu/catablast/blast
#   - run `blast gen all` so generated dirs are repopulated against the latest blast
#
# Usage:
#   ./blast-reset.sh                 # defaults to ./catalyst
#   ./blast-reset.sh tweetbook       # any project under catablast/
#   ./blast-reset.sh /abs/path       # absolute path also works
#
# Refuses to delete anything outside the named project root.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BLAST_SRC="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKSPACE="$(cd "$BLAST_SRC/.." && pwd)"
TARGET="${1:-catalyst}"

case "$TARGET" in
    /*) PROJECT="$TARGET" ;;
    *)  PROJECT="$WORKSPACE/$TARGET" ;;
esac

if [[ ! -f "$PROJECT/Cargo.toml" ]]; then
    echo "blast-reset: $PROJECT is not a project root (no Cargo.toml)" >&2
    exit 1
fi

cd "$PROJECT"
echo "blast-reset: target = $PROJECT"

echo "blast-reset: wiping <layer>/generated/ dirs"
# Hard-coded to known generated dirs to refuse anything stray.
GENERATED_DIRS=(
    "src/structs/generated"
    "src/database/generated"
    "src/services/generated"
    "src/models/generated"
    "src/routines/generated"
    "src/flows/generated"
    "src/transport/http/generated"
    "src/transport/ws/generated"
    "src/transport/leptos/pages/generated"
    "src/transport/leptos/routes/generated"
    "src/transport/leptos/data/generated"
    "src/views/components/generated"
)
for rel in "${GENERATED_DIRS[@]}"; do
    rm -rf "$rel"
    # Always leave a placeholder so the project-owned `pub mod generated;`
    # in the parent barrel resolves even when blast emits nothing for the layer.
    mkdir -p "$rel"
    : > "$rel/mod.rs"
    echo "  reset $rel"
done

if [[ -f tests/route_alignment_generated.rs ]]; then
    rm -f tests/route_alignment_generated.rs
    echo "  dropped tests/route_alignment_generated.rs"
fi

echo "blast-reset: reinstalling blast from $BLAST_SRC"
cargo install --path "$BLAST_SRC" --quiet

echo "blast-reset: blast gen all"
blast gen all

echo "blast-reset: done — $PROJECT"
