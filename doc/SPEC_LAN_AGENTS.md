# SPEC_LAN_AGENTS

LAN-server-offloaded agent worktree workflow. When orchestrating ≥3 parallel coding agents from one Claude Code session, local CPU melts: each agent fires `cargo check`/`cargo build` in its own worktree, all 56 cores fight each other, the laptop becomes 1fps. This spec captures the offload setup that makes 7+ parallel agents tractable.

## Architecture

```
┌─ laptop ──────────────────────┐         ┌─ LAN box (192.168.0.155) ──┐
│                               │         │                            │
│  Claude Code orchestrator     │         │  56-core / 86GB RAM        │
│  Agent worktrees:             │         │                            │
│   blast/.claude/worktrees/    │         │  /srv/wt/ (tmpfs 64GB)     │
│   ├── agent-A/                │ rcargo  │  ├── canonical-<sha>/      │
│   ├── agent-B/                │  rsync  │  ├── canonical-<sha>/      │
│   ├── agent-C/                │ ─────►  │  └── ...                   │
│   └── ...                     │         │                            │
│                               │  ssh    │  valkey :6379 (sccache)    │
│  ~/bin/rcargo  ◄──────────────┼─────────┤  ~/.cargo/config.toml      │
│  excludes target/, .cargo/    │  cargo  │   rustc-wrapper=sccache    │
│                               │  output │   linker=mold              │
└───────────────────────────────┘         └────────────────────────────┘
```

Each agent's `Bash` tool calls `rcargo` instead of `cargo`. The wrapper rsyncs the worktree to the server, runs cargo there, streams output back. `target/` lives on the server's tmpfs only — never touches local disk or the LAN.

## Server setup (one-time)

Arch host, user `tragdate`, ssh key `~/.ssh/sri_key`, NOPASSWD sudo.

```bash
ssh -i ~/.ssh/sri_key tragdate@192.168.0.155 'sudo pacman -Syy --noconfirm && sudo pacman -S --needed --noconfirm rustup redis mold clang postgresql-libs'
ssh ... 'rustup default stable'
ssh ... 'sudo systemctl enable --now valkey'   # arch ships valkey, redis-protocol-compat
```

Server-side `~/.cargo/config.toml`:

```toml
[build]
rustc-wrapper = "sccache"

[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

Server-side `~/.config/sccache.env` (sourced from `~/.zshenv` and `~/.zshrc`):

```bash
export RUSTC_WRAPPER=sccache
export SCCACHE_REDIS=redis://127.0.0.1:6379
export SCCACHE_CACHE_SIZE=50G
```

`/srv/wt/` is a 64GB tmpfs (mounted in `/etc/fstab`). Worktree mirrors land here; reboots wipe them, sccache redis persists.

```fstab
tmpfs  /srv/wt  tmpfs  size=64G,nr_inodes=10M,mode=1777  0 0
```

## Local: rcargo wrapper

`~/bin/rcargo` (`~/bin` in `$PATH` via `~/.config/zsh/environment.zsh`):

```bash
#!/usr/bin/env bash
set -e

REMOTE_HOST="${RCARGO_HOST:-192.168.0.155}"
REMOTE_USER="${RCARGO_USER:-tragdate}"
REMOTE_KEY="${RCARGO_KEY:-$HOME/.ssh/sri_key}"
REMOTE_ROOT="${RCARGO_ROOT:-/srv/wt}"

# Walk up for Cargo.toml.
project_root=$(pwd)
while [[ "$project_root" != "/" && ! -f "$project_root/Cargo.toml" ]]; do
    project_root=$(dirname "$project_root")
done
[[ -f "$project_root/Cargo.toml" ]] || { echo "rcargo: no Cargo.toml above $(pwd)" >&2; exit 1; }

# Stable per-path slug → per-worktree remote dir + target/.
slug=$(echo -n "$project_root" | sha1sum | cut -c1-12)
remote_dir="$REMOTE_ROOT/$(basename "$project_root")-${slug}"
ssh_opts=(-i "$REMOTE_KEY" -o ControlMaster=auto -o ControlPath=/tmp/rcargo-%C -o ControlPersist=600)

ssh "${ssh_opts[@]}" "$REMOTE_USER@$REMOTE_HOST" "mkdir -p $remote_dir" >/dev/null

rsync -a --delete \
    --exclude='/target/' --exclude='/target-*/' \
    --exclude='/node_modules/' --exclude='/.git/objects' --exclude='/.git/lfs' \
    --exclude='/.cargo/' --exclude='/dist/' --exclude='/.vite/' \
    --exclude='/storage/logs/' --exclude='*.log' \
    -e "ssh ${ssh_opts[*]}" \
    "$project_root/" \
    "$REMOTE_USER@$REMOTE_HOST:$remote_dir/"

rel_pwd=$(realpath --relative-to="$project_root" "$(pwd)")
quoted_args=$(printf '%q ' "$@")

exec ssh -t "${ssh_opts[@]}" "$REMOTE_USER@$REMOTE_HOST" \
    "cd $remote_dir/$rel_pwd && source ~/.config/sccache.env 2>/dev/null; export PATH=\$HOME/.cargo/bin:\$PATH; cargo $quoted_args"
```

The `.cargo/` exclude matters: canonical templates ship a `.cargo/config.toml` that redirects `target-dir = "../../target/canonical"` for the in-place dev loop on local — that path resolves to `/srv` on the server, which the user can't write. Excluding it keeps the server falling back to the default `target/` inside the remote worktree dir.

## Performance numbers (measured)

Cold first build of `blast` (heavy crate: diesel, axum, tokio, leptos full-stack):
- **1m46s** — sccache redis empty, populates 223 dep rlibs.

Subsequent worktrees (different per-path slug, fresh tmpfs target/) hitting same dep versions:
- **30s** — sccache pulls 223 rlibs from redis, compiles only the local crate.

Incremental rebuild on the same worktree:
- **<1s** — cargo's own incremental cache hits, no recompile.

For a 7-agent wave with all hitting similar dep graphs, total compile cost across the LAN box is ~1m46s + 6×~30s ≈ 6m parallelized across 56 cores. Local CPU usage: ~0%.

## Allowlist for agents

Project-level `.claude/settings.json` allows rcargo invocations without permission prompts (sub-agents otherwise hit the prompt loop and falsely conclude they're sandbox-locked):

```json
{
  "permissions": {
    "allow": [
      "Bash(rcargo check)",
      "Bash(rcargo check *)",
      "Bash(rcargo build)",
      "Bash(rcargo build *)",
      "Bash(rcargo test)",
      "Bash(rcargo test *)",
      "Bash(rcargo clippy *)",
      "Bash(cd ../catalyst && rcargo *)"
    ]
  }
}
```

Don't broaden to `Bash(rcargo *)` — `rcargo run --release` on heavy crates can spike RAM (27GB+ for blast in release mode) and nothing in this stack benefits from running binaries on the LAN box.

## Wave-spawn protocol

When orchestrator launches N agents in parallel via the Agent tool with `isolation: "worktree"`:

1. **Commit master state cleanly first.** Worktree forks happen off `origin/master`. Update the local ref to match `master` HEAD without pushing:
   ```bash
   git update-ref refs/remotes/origin/master refs/heads/master
   ```
   Otherwise new worktrees fork off whatever `origin/master` points to (often a stale snapshot of upstream), and agents work against ancient code.

2. **Embed in every agent prompt:**
   - "Use `rcargo` (NOT `cargo`) for ALL build/check/test."
   - "Don't pipe cargo output through grep/head/tail/awk/sed/wc — read full output. (see root CLAUDE.md CARGO LAW section.)"
   - "Atomic commits, ≤72 char subjects, no Co-Authored-By, don't push."
   - "Layer rules in `catalyst/build.rs` (LEPTOS:1-10, ERROR, DEAD families) panic the build on violation."
   - "Don't enter any `/command` or skill — you are NOT in any skill mode." (Defensive against agents falling into `/fewer-permission-prompts` loops.)

3. **Cherry-pick on completion.** Agents commit on their own branch (`worktree-agent-<id>`); orchestrator cherry-picks the new commits onto master, resolves conflicts (typically `mod.rs` barrels — append modules from both sides), then runs `rcargo check` to verify.

4. **Cleanup.** After cherry-pick:
   ```bash
   git worktree remove -f -f .claude/worktrees/agent-<id>
   git branch -D worktree-agent-<id>
   git update-ref refs/remotes/origin/master refs/heads/master
   ```

## Failure modes

| Symptom | Cause | Fix |
|---------|-------|-----|
| Agent reports "permission denied" on Bash/Write | hit `/fewer-permission-prompts` skill loop on `rcargo` denial | add rcargo entries to `.claude/settings.json`; respawn |
| Agent forks off ancient master | `origin/master` points to stale upstream | `git update-ref refs/remotes/origin/master refs/heads/master` |
| `rcargo` fails with "Permission denied" on `/srv/wt/.../target` | canonical's `.cargo/config.toml` redirects target outside the writable area | rsync excludes `/.cargo/`; wipe `/srv/wt/<worktree>/.cargo/` once if previously synced |
| Multiple worktree commits conflict on `mod.rs` | each agent appended to the same barrel | resolve by combining all `pub mod`/`pub use` lines into one canonical sort |
| Agent prompt rejected with "too long" | over 8KB prompt | trim verbose context, reference doc paths instead of inlining |
| `rcargo` shows file lock waits | concurrent waves hit cargo's package-cache lock | harmless; cargo serializes index updates per server, builds proceed in parallel |

## Why not git-based offload

- `git push` to a server's bare repo + remote checkout: requires the server to know about every PR and slows commit cadence.
- NFS/sshfs mount of the worktree: filesystem latency murders rustc on small-file IO.
- Distributed compilers (icecc, distcc): not maintained for rustc.
- `cargo-zigbuild` cross-compile: doesn't move the actual rustc work; just the linker.

`rsync + ssh + sccache-redis + tmpfs target/` is the simplest combination that actually moves the CPU work without giving up local-edit ergonomics.

## Multi-claude on the same machine

`CLAUDE_CONFIG_DIR=~/.claude-alt claude -p ...` runs a separate claude with isolated session/state. Useful when:
- You want two interactive top-level threads (one driving blast, one driving canonical) that share the same project state.
- A wave-spawn orchestrator becomes the bottleneck and you want a sibling thread doing other work.

For single-orchestrator + N parallel agents, the Agent tool with `isolation: "worktree"` is sufficient — no extra claude instances needed.

## Related

- `blast/CLAUDE.md` — local dev loop, install.sh, blast tool architecture.
- `catalyst/CLAUDE.md` — canonical app layer rules + lint family.
- `root CLAUDE.md` (root) — wave-spawn protocol summary, no-grep-cargo law.
