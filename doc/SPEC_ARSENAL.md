# SPEC_ARSENAL

Capability inventory tool. Walks the user's app, extracts every `pub fn` across layers, emits a machine-readable map. Purpose: agents (and humans) see what already exists before writing anything new.

## Role

Pairs with the strict layered architecture + Primer-driven codegen to eliminate "AI slop" failure modes:

- AI re-implements a utility that already exists → Arsenal shows it, AI reuses
- AI writes business logic in a route → layer rules + `cargo check` reject
- AI duplicates pagination/validation/crypto code → it's already codegen'd or in services
- AI hallucinates a function signature → Arsenal has the exact sig

Combined with `blast check` (FE lint) + `cargo check` + layer-enforced imports, the framework actively refuses slop.

## Invocation

- `blast arsenal` — scans source, writes `target/arsenal.json`
- `blast arsenal serve` — serves same data over MCP stdio protocol (for AI agents)
- Auto-regenerated as a post-pass in `blast gen all`
- Regenerated on source change by `blast watch`

## What Gets Scanned

| Layer | Extracts |
|-------|----------|
| `services/` | `pub fn` signatures, doc comments, side-effect class (pure / io / net / db) |
| `routines/` | `pub fn` signatures, grouped by subfolder (act / collect / derive) |
| `models/` | `pub fn` signatures, resource ownership |
| `flows/` | `pub fn` signatures, inputs/outputs, retry presence (Crank detected) |
| `transport/` | route → flow mapping (http/ws/fuses entry points) |

NOT scanned: `src/structs/`, `src/database/`, `frontend/` — those are shape/presentation, not capabilities.

Both `generated/` and `custom/` subtrees are indexed.

## Output Schema (`target/arsenal.json`)

```json
{
  "generated_at": "2026-04-24T12:00:00Z",
  "layers": {
    "services": [
      {
        "module": "email",
        "name": "send",
        "fqn": "services::email::send",
        "signature": "async fn send(to: &str, subject: &str, body: &str) -> Result<(), MeltDown>",
        "doc": "Sends plain-text email via SMTP.",
        "side_effects": ["net"],
        "origin": "custom"
      }
    ],
    "routines": [ /* ... */ ],
    "flows": [ /* ... */ ],
    "models": [ /* ... */ ],
    "transport": [ /* { "method": "GET", "path": "/api/orders", "flow": "flows::orders::list" } */ ]
  }
}
```

## Parser

- `syn` walks each `*.rs` file
- Only `pub fn` / `pub async fn` extracted
- Doc comment preserved (first paragraph)
- Side-effect heuristic: function signature + use declarations (diesel → db, reqwest/hyper → net, tokio::fs → io)
- Origin field: `generated` if under `layer/generated/`, else `custom`
- Cheap. Target <1s for a medium-sized app.

## MCP Server Mode

`blast arsenal serve` implements the Model Context Protocol stdio transport.

Tools exposed:
- `list(layer?: string)` — returns all entries, optionally filtered by layer
- `search(query: string)` — fuzzy match on fn name + doc + fqn
- `describe(fqn: string)` — full signature + doc + location for a specific fn
- `routes()` — returns transport-layer route → flow mapping

Agents that speak MCP (Claude Code, etc.) add Arsenal as a tool source. The AI can query capabilities before authoring a feature.

Suggested agent pre-flight:
1. `search("email")` — see existing email-related functions
2. `describe("flows::email::send_notification")` — get full sig
3. Compose new flow from existing primitives; don't reinvent

## Determinism

Output is deterministic across runs with identical source (sorted layer entries, stable ordering). Same input → byte-identical `arsenal.json`.

## Related Specs

- `catalyst/doc/SPEC_ARCHITECTURE.md` — layer boundaries Arsenal indexes
- `catalyst/doc/SPEC_FLOWS.md` — flows are the primary capability surface
- `SPEC_BLAST_COMMANDS.md` — full CLI surface
- `SPEC_CODEGEN.md` — Arsenal regenerates post-codegen
