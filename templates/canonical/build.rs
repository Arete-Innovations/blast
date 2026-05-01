use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const MAX_LOC: usize = 800;

const SILENT_ERROR_PATTERNS: &[&str] = &[
    ".unwrap()",
    ".unwrap_or(",
    ".unwrap_or_default()",
    ".unwrap_or_else(",
    ".expect(",
    ".ok()",
    ".map_or(",
    ".map_or_else(",
    ".or(",
    ".or_else(",
    "let _ =",
    "_ =>",
    "if let Some(",
    "if let Ok(",
    "while let Some(",
    "while let Ok(",
];

const RESULT_OPTION_MATCH_ARMS: &[&str] = &["Some(_) =>", "Ok(_) =>", "Err(_) =>"];

const ERR_DEFAULT_VALUES: &[&str] = &[
    "=> None",
    "=> 0",
    "=> false",
    "=> true",
    "=> \"\"",
    "=> String::new()",
    "=> Vec::new()",
    "=> HashMap::new()",
    "=> HashSet::new()",
    "=> BTreeMap::new()",
    "=> BTreeSet::new()",
    "=> Default::default()",
    "=> Ok(None)",
    "=> Ok(0)",
    "=> Ok(false)",
    "=> Ok(true)",
    "=> Ok(\"\")",
    "=> Ok(String::new())",
    "=> Ok(Vec::new())",
    "=> Ok(HashMap::new())",
    "=> Ok(Default::default())",
];

const LAYER_NAMES: &[&str] = &["transport", "flows", "routines", "models", "services", "database", "structs"];

struct Hit {
    rule: &'static str,
    file: String,
    line: usize,
}

fn category_for(rule: &str) -> &'static str {
    if rule.starts_with("DECOMPOSITION:") {
        return "DECOMPOSITION";
    }
    if rule.starts_with("ERROR:") {
        return "ERROR";
    }
    if rule.starts_with("TYPE:") {
        return "TYPE";
    }
    if rule.starts_with("DEAD:") {
        return "DEAD";
    }
    if rule.starts_with("LAYER:") {
        return "LAYER";
    }
    if rule.starts_with("STRUCTS:") {
        return "STRUCTS";
    }
    if rule.starts_with("TRANSPORT:") {
        return "TRANSPORT";
    }
    if rule.starts_with("LEPTOS:") {
        return "LEPTOS";
    }
    "OTHER"
}

fn category_spirit(cat: &str) -> &'static str {
    match cat {
        "DECOMPOSITION" => "every file has one job. if it outgrew that job, split it.",
        "ERROR" => "propagate or crash. there is no third option. if an operation failed, the caller must know.",
        "TYPE" => "the type signature is the contract. if the error type is erased, the contract is worthless.",
        "DEAD" => "code that isn't serving the program right now is noise. noise misleads. delete it.",
        "LAYER" => "the chain is law. transport → flow → routine → models/services → database. only models reach the basement. you may import down, never up, never sideways across siblings.",
        "STRUCTS" => "data shapes belong in src/structs/. behavior layers are for behavior; defining types inline scatters the data model and gives codegen one more place to look.",
        "TRANSPORT" => "every http handler runs through `request_ctx_middleware`, which builds a per-request `Ctx` (with session loaded if a token was sent) and inserts it as `Extension<Ctx>`. handlers MUST extract `Extension<Ctx>` so `ctx.require_session()` sees the loaded session. `State<Ctx>` returns the global anonymous ctx — silently fails auth.",
        "LEPTOS" => "the leptos UI is the BE's dumb relayer. styling lives in tokens + module scss; pages wear PageShell. inline styles, hex colors, raw px, and naked page bodies are drift — caught here before they spread.",
        _ => "",
    }
}

fn rule_help(rule: &str) -> &'static str {
    match rule {
        "DECOMPOSITION:1" => "split by responsibility, not by arbitrary cut. if a file does two things, make two files.",
        "DECOMPOSITION:2" => "lib.rs and mod.rs are wiring — mod, use, attributes. move logic to its own file.",
        "ERROR:3" => concat!(
            ".unwrap(), .expect(), .ok(), .or(), if let Some/Ok, let _ =, _ => are all banned.\n",
            "    the ONLY legal moves: propagate with ? or return Err() explicitly.\n",
            "    there are exactly two honest responses to failure: tell the caller, or crash.",
        ),
        "ERROR:4" => concat!(
            "Ok(_), Err(_), Some(_) all throw away the value you're matching on.\n",
            "    bind it: Ok(val), Err(err), Some(val). if you don't need the value, you don't need the match.\n",
            "    `None =>` is allowed (no inner value to bind); silent-swallow via `None` is still caught by ERROR:5/6/18.",
        ),
        "ERROR:5" => concat!(
            "returning Vec::new(), 0, None, false, or Default::default() from an error arm is lying.\n",
            "    the operation failed. say so. return Err and let the caller decide what 'empty' means.",
        ),
        "ERROR:6" => concat!(
            "an Err arm that does nothing is the #1 source of 'it silently stopped working' bugs.\n",
            "    you received an error, inspected it, and chose to pretend it didn't happen.",
        ),
        "TYPE:7" => concat!(
            "Result<T, ?> must carry a named, non-erased error type. erasure is banned:\n",
            "    String, &str, Box<dyn ...>, dyn ..., anyhow::Error, eyre::Report, (), bare primitives,\n",
            "    and lone single-letter generics. name your type — its identity IS the contract.",
        ),
        "DEAD:8" => concat!(
            "#[allow(dead_code/unused)] tapes over the check engine light.\n",
            "    the compiler says it's dead. trust it. delete the code — git has it if you ever need it back.",
        ),
        "DEAD:9" => concat!(
            "commented-out code rots, misleads, and makes grep lie to you.\n",
            "    version control exists. delete it. if you want it back: git log.",
        ),
        "DEAD:10" => concat!(
            "todo!(), unimplemented!(), unreachable!() are runtime panics wearing a trenchcoat.\n",
            "    either implement it now or remove the function. half-built code that compiles is worse than missing code.",
        ),
        "LAYER:11" => concat!(
            "transport is the thin entry point. it calls flows ONLY.\n",
            "    forbidden imports under src/transport/: crate::routines, crate::models, crate::services, crate::database.\n",
            "    one handler → one flow call → response. no shortcuts, no business branching.",
        ),
        "LAYER:12" => concat!(
            "flows compose routines under a Crank policy. they are the capability inventory.\n",
            "    forbidden imports under src/flows/: crate::models, crate::services, crate::database, crate::transport, crate::flows.\n",
            "    no flow → flow either: shared work belongs in a routine, not in another flow.",
        ),
        "LAYER:13" => concat!(
            "routines are atomic capabilities. they call models and services for ONE business action.\n",
            "    forbidden imports under src/routines/: crate::database, crate::flows, crate::transport, crate::routines.\n",
            "    no routine → routine: chained ops are flows. no direct database: DB ops go through models, conn handed in via &Ctx.",
        ),
        "LAYER:14" => concat!(
            "models are the only layer that talks to database. SQL lives here.\n",
            "    forbidden imports under src/models/: crate::services, crate::routines, crate::flows, crate::transport.\n",
            "    take a Connection or transaction handle, run queries, return Result<T, MeltDown>.",
        ),
        "LAYER:15" => concat!(
            "services are stateless single-shot adapters: crypto, email, storage, external HTTP.\n",
            "    forbidden imports under src/services/: crate::database, crate::models, crate::routines, crate::flows, crate::transport.\n",
            "    no retry — that's the flow's job. structs only.",
        ),
        "LAYER:16" => concat!(
            "database is the basement: pool, migrations, schema.\n",
            "    forbidden imports under src/database/: crate::models, crate::services, crate::routines, crate::flows, crate::transport.\n",
            "    structs only.",
        ),
        "LAYER:17" => concat!(
            "structs are inert data definitions. they depend on nothing inside the crate.\n",
            "    forbidden imports under src/structs/: crate::transport, crate::flows, crate::routines, crate::models, crate::services, crate::database.\n",
            "    stdlib + external crates only.",
        ),
        "ERROR:18" => concat!(
            "a bound `Err(<name>) =>` arm that doesn't log via cata_log! / .log() and doesn't propagate via return Err(...) / Err(...) / MeltDown::* / ? is silent swallowing.\n",
            "    log it with cata_log!(Error, format!(\"...: {}\", e)) before recovering, or use ? instead of a manual match if you only want propagation.",
        ),
        "ERROR:19" => concat!(
            "`let _<ident> = expr;` is the canonical 'shut up unused warning' pattern — but for Result-returning calls it's silent error discard.\n",
            "    use ? to propagate, or match with explicit cata_log! if you want to recover. RAII drop guards: bind with explicit type annotation `let _guard: SomeGuard = ...;` to opt out.",
        ),
        "ERROR:20" => concat!(
            "`.map_err(|_| ...)` discards the original error's context — source, message, chain all lost.\n",
            "    bind it: `.map_err(|e| MeltDown::from(e))` or `.map_err(|e| MeltDown::new(...).with_source(e))`.",
        ),
        "DEAD:21" => concat!(
            "every form of comment (//, ///, //!, /* */, /** */, /*! */) is banned in src/.\n",
            "    names, types, and commit messages are the documentation. if a reader needs a comment to follow the code, refactor: smaller fns, better names, named constants.\n",
            "    one exception: files under a `generated/` segment may have a leading comment header (auto-gen warning + hash marker) BEFORE any non-comment code. once code starts, the rest of the file is back under \
             the total ban.",
        ),
        "STRUCTS:22" => concat!(
            "data definitions (`struct`, `enum`) live in src/structs/.\n",
            "    declaring `struct` or `enum` inside a layer dir scatters the data model. move the type to src/structs/<resource>/<file>.rs and import it where needed.\n",
            "    exempt: src/structs/, src/meltdown.rs, src/ctx.rs, src/crank.rs, src/cata_log.rs, src/bootstrap.rs, src/lib.rs, src/main.rs, src/database/schema.rs.",
        ),
        "TRANSPORT:23" => concat!(
            "http handler signatures must use `Extension(ctx): Extension<Ctx>` — never `State(ctx): State<Ctx>`.\n",
            "    `request_ctx_middleware` builds the per-request `Ctx` (with session) and inserts it as Extension. `State<Ctx>` returns the global anonymous ctx — `ctx.require_session()` then returns `None` and every protected route silently 401s.\n",
            "    exempt: src/transport/http/middleware/ (middleware legitimately extracts State<Ctx> to build the per-request Ctx).",
        ),
        "LEPTOS:1" => concat!(
            "inline `style=` attributes inside `view!` macros are banned in src/transport/leptos/.\n",
            "    styling belongs in per-component `.module.scss` via stylance + design tokens (`var(--app-*)`).\n",
            "    fix: define a class in `<file>.module.scss`, reference it via `class=style::FOO`.",
        ),
        "LEPTOS:2" => concat!(
            "raw color literals (`#abc`, `#abcdef`, `rgb(...)`, `rgba(...)`, `hsl(...)`, `hsla(...)`) are banned in src/transport/leptos/ and src/structs/.\n",
            "    color values live in `style/tokens.scss` only. consume via `var(--app-color-*)` from a module scss file.",
        ),
        "LEPTOS:3" => concat!(
            "raw `px` units are banned outside `style/tokens.scss` and `style/base.scss`.\n",
            "    use rem-scaled tokens (`var(--app-space-*)`, `var(--app-fs-*)`) instead. allowed exceptions: `0.0625rem` hairlines and `@media` query breakpoints.",
        ),
        "LEPTOS:4" => concat!(
            "every page component (file under src/transport/leptos/pages/) must wrap its top-level `view!` in `<PageShell layout=...>`.\n",
            "    pages own no chrome — the shell does. add `<PageShell layout=PageLayout::Cards>...</PageShell>` (or wrap inside `<AuthGuard>` for protected pages).",
        ),
        "LEPTOS:5" => concat!(
            "hardcoded route paths in `nav(...)` first-arg, `<a href=\"/...\">`, or `<A href=\"/...\">` are banned in src/transport/leptos/.\n",
            "    use `RouteName::*.path()` (or `.as_ref()`) instead. compile-checks the route against the typed enum so renames don't silently break links.\n",
            "    allowed externals: `\"#\"`, `mailto:`, `tel:`, `https://`, `//`. lines that already mention `RouteName::` are skipped.",
        ),
        "LEPTOS:6" => concat!(
            "looks like an optimistic update — a list/map signal mutated between `pending.set(true)` and `spawn_local(`.\n",
            "    mutate signals only inside the `match result {}` block AFTER server response — single source of truth lives on the BE.\n",
            "    heuristic; false positives possible. if this hit is wrong, restructure the function so `pending.set(true)` and the unrelated `.set/.update(...)` aren't on adjacent lines, or land the actual mutation behind the spawn_local boundary.",
        ),
        "LEPTOS:7" => concat!(
            "`\"Loading...\"` literal outside an `Option::None =>` arm or `Suspense fallback=...` is a stale-spinner smell.\n",
            "    after first load, refetch silently. use `RwSignal<Option<...>>` + cfg-gated Effect; SSR placeholder only on cold-load. once you have data, never blank the screen on refresh.",
        ),
        "LEPTOS:8" => concat!(
            "`ListQuery::default()` outside src/transport/leptos/signals/url.rs scatters list state into local components.\n",
            "    use `use_url_list_state()` (signals/url.rs) so pagination/sort/filter live in the URL — refresh-survivable, back-button correct, deep-linkable.",
        ),
        "LEPTOS:9" => concat!(
            "`RwSignal::new(false)` adjacent (≤5 lines) to a `dialog`/`drawer`/`modal`/`popup`-named identifier looks like local dialog state.\n",
            "    use `use_query_dialog(name)` so dialog open/close persists in the URL — refresh-survivable, back-button closes the dialog correctly.\n",
            "    heuristic; false positives possible. if this hit is wrong, rename the binding so it doesn't carry a dialog/modal token, or move the `RwSignal::new(false)` away from any identifier matching that vocabulary.",
        ),
        "LEPTOS:10" => concat!(
            "form-control selector (`input`/`select`/`textarea`/`button`) inside a `.module.scss` declares `font-size:` without referencing `var(--app-fs-*)` or `inherit`.\n",
            "    UA-default form-control fonts bypass the rem-scaled root and stay tiny at 4K. base.scss already pins these to `var(--app-fs-md)` + `font: inherit` — per-component overrides MUST keep that contract.\n",
            "    fix: use `font-size: var(--app-fs-md)` (or any other `--app-fs-*` token) or `font-size: inherit`.",
        ),
        _ => "",
    }
}

const CATEGORY_ORDER: &[&str] = &["DECOMPOSITION", "LAYER", "STRUCTS", "TRANSPORT", "LEPTOS", "ERROR", "TYPE", "DEAD"];

fn format_report(hits: &[Hit]) -> String {
    let mut by_rule: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for hit in hits {
        by_rule.entry(hit.rule).or_default().push(format!("{}:{}", hit.file, hit.line));
    }

    let mut by_category: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for rule in by_rule.keys() {
        let cat = category_for(rule);
        let rules = by_category.entry(cat).or_default();
        if !rules.contains(rule) {
            rules.push(rule);
        }
    }

    let mut out = String::new();
    out.push_str(&format!("\n{} violations\n", hits.len()));

    for cat in CATEGORY_ORDER {
        let rules = match by_category.get(cat) {
            Some(r) => r,
            None => continue,
        };

        out.push_str(&format!("\n--- {} --- {}\n", cat, category_spirit(cat)));

        for rule in rules {
            let locations = match by_rule.get(rule) {
                Some(l) => l,
                None => continue,
            };

            out.push_str(&format!("\n  [{}] {}\n", rule, locations.join(" | ")));
            out.push_str(&format!("    {}\n", rule_help(rule)));
        }
    }

    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // LAYER:1x resolver self-test. Runs on every canonical build. Cheap (~µs).
    // If anyone breaks `module_path_for_rel` / `resolve_to_crate_path`, the
    // build hard-fails here BEFORE silently letting layer escapes through.
    verify_layer_resolver_invariants();

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut hits: Vec<Hit> = Vec::new();

    bootstrap_stylance_index(&manifest_dir);

    if src_dir.is_dir() {
        scan_dir(&manifest_dir, &src_dir, &mut hits);
    }

    if !hits.is_empty() {
        panic!("\n{}", format_report(&hits));
    }
}

fn verify_layer_resolver_invariants() {
    // module_path_for_rel — file path → canonical module segments.
    let cases_mp: &[(&str, &[&str])] = &[
        ("flows/auth/login.rs", &["crate", "flows", "auth", "login"]),
        ("flows/auth/mod.rs", &["crate", "flows", "auth"]),
        ("flows.rs", &["crate", "flows"]),
        ("models/auth/users.rs", &["crate", "models", "auth", "users"]),
        ("transport/leptos/pages/welcome.rs", &["crate", "transport", "leptos", "pages", "welcome"]),
    ];
    for (rel, expected) in cases_mp {
        let got = module_path_for_rel(&PathBuf::from(rel));
        let want: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(got, want, "module_path_for_rel({:?}) wrong", rel);
    }

    // resolve_to_crate_path — anchored to a flows/auth/login.rs viewpoint.
    let mp_login: Vec<String> = ["crate", "flows", "auth", "login"].iter().map(|s| s.to_string()).collect();
    let cases_resolve: &[(&str, Option<&str>)] = &[
        // Absolute crate paths passthrough.
        ("crate", Some("crate")),
        ("crate::models::Foo", Some("crate::models::Foo")),
        ("crate::flows::*", Some("crate::flows::*")),
        // self::
        ("self", Some("crate::flows::auth::login")),
        ("self::Bar", Some("crate::flows::auth::login::Bar")),
        // super:: chains — this is the LAYER bypass FIX-010 closes.
        ("super", Some("crate::flows::auth")),
        ("super::Foo", Some("crate::flows::auth::Foo")),
        ("super::super::models::Foo", Some("crate::flows::models::Foo")),
        ("super::super::super::models::Foo", Some("crate::models::Foo")),
        ("super::super::super", Some("crate")),
        // 4 supers from len-4 mod_path overshoots crate root → None.
        ("super::super::super::super", None),
        // External imports — must return None so the LAYER ban skips them.
        ("std::fmt::Display", None),
        ("serde::Deserialize", None),
        ("leptos::prelude::*", None),
        // Names that LOOK like super/self/crate but aren't — must be None.
        ("supersize::X", None),
        ("selfless::Y", None),
        ("crateful::Z", None),
        // Walking past crate root → None.
        // From mp_login (len 4): super × 5 = invalid. We have super::super::super::super::super.
        ("super::super::super::super::super", None),
    ];
    for (path, expected) in cases_resolve {
        let got = resolve_to_crate_path(path, &mp_login);
        let want = expected.map(|s| s.to_string());
        assert_eq!(got, want, "resolve_to_crate_path({:?}) wrong", path);
    }

    // From a shorter mod_path (len 2): super::super → walks past crate root → None.
    let mp_flows: Vec<String> = ["crate", "flows"].iter().map(|s| s.to_string()).collect();
    let edge_got = resolve_to_crate_path("super::super", &mp_flows);
    assert_eq!(edge_got, None, "super::super from len-2 mod_path must overshoot to None");
}

fn bootstrap_stylance_index(manifest_dir: &Path) {
    let dir = manifest_dir.join("style").join("generated");
    let bundle = dir.join("stylance.scss");
    let _ = fs::create_dir_all(&dir);

    if let Ok(status) = std::process::Command::new("stylance").arg(manifest_dir).status() {
        if status.success() {
            return;
        }
    }

    if !bundle.exists() {
        let _ = fs::write(&bundle, "// stylance-cli not found — install via `cargo install stylance-cli`.\n");
    }
}

fn hit(hits: &mut Vec<Hit>, rule: &'static str, file: &Path, line: usize) {
    hits.push(Hit {
        rule,
        file: file.to_string_lossy().to_string(),
        line,
    });
}

fn scan_dir(manifest_dir: &Path, dir: &Path, hits: &mut Vec<Hit>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            scan_dir(manifest_dir, &path, hits);
        } else if path.extension().map_or(false, |e| e == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
            scan_file(manifest_dir, &path, hits);
        } else if is_module_scss(&path) {
            println!("cargo:rerun-if-changed={}", path.display());
            scan_module_scss(manifest_dir, &path, hits);
        }
    }
}

fn is_module_scss(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    name.ends_with(".module.scss")
}

fn scan_file(manifest_dir: &Path, path: &Path, hits: &mut Vec<Hit>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let src_dir = manifest_dir.join("src");
    let rel = path.strip_prefix(&src_dir).unwrap_or(path);
    let rel_str = rel.to_string_lossy();
    let file_name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();

    let is_bin = rel_str.starts_with("bin/") || file_name == "main.rs";

    let loc = content.lines().count();
    if loc > MAX_LOC {
        hit(hits, "DECOMPOSITION:1", rel, 0);
    }

    let is_lib_or_mod = file_name == "lib.rs" || file_name == "mod.rs";
    if is_lib_or_mod {
        check_switchboard_purity(rel, &content, hits);
    }

    let test_mask = cfg_test_line_mask(&content);
    let mut in_block_comment = false;
    let mut in_macro_rules = false;
    let mut macro_brace_depth: i32 = 0;

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if test_mask[line_no] {
            continue;
        }

        if in_macro_rules {
            macro_brace_depth += trimmed.chars().filter(|&c| c == '{').count() as i32;
            macro_brace_depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            if macro_brace_depth <= 0 {
                in_macro_rules = false;
            }
            continue;
        }
        if !in_block_comment && !trimmed.starts_with("//") && trimmed.starts_with("macro_rules!") {
            in_macro_rules = true;
            macro_brace_depth = trimmed.chars().filter(|&c| c == '{').count() as i32;
            macro_brace_depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            if macro_brace_depth <= 0 {
                in_macro_rules = false;
            }
            continue;
        }

        if in_block_comment {
            continue;
        }

        if trimmed.starts_with("//") {
            check_commented_out_code(rel, line_no, trimmed, hits);
            continue;
        }

        check_silent_errors(rel, line_no, trimmed, hits);
        check_wildcard_match_arms(rel, line_no, trimmed, hits);
        check_err_default_arms(rel, line_no, trimmed, hits);
        check_dead_code_suppression(rel, line_no, trimmed, hits);
        check_unfinished_markers(rel, line_no, trimmed, is_bin, hits);
        check_underscore_bind(rel, line_no, trimmed, hits);
        check_map_err_discard(rel, line_no, trimmed, hits);
        check_underscore_pattern_bindings(rel, line_no, trimmed, hits);
    }

    check_empty_err_arms(rel, &content, hits);
    check_result_types(rel, &content, hits);
    check_layer_imports(rel, &content, hits);
    check_err_arm_handling(rel, &content, hits);
    check_no_comments(rel, &content, hits);
    check_inline_data_definitions(rel, &content, hits);
    check_handler_state_ctx(rel, &content, hits);
    check_leptos_inline_style(rel, &content, hits);
    check_leptos_hex_colors(rel, &content, hits);
    check_leptos_px_units(rel, &content, hits);
    check_leptos_page_shell_required(rel, &content, hits);
    check_leptos_hardcoded_route_path(rel, &content, hits);
    check_leptos_optimistic_update_in_custom(rel, &content, hits);
    check_leptos_loading_spinner_after_first_load(rel, &content, hits);
    check_leptos_local_list_state(rel, &content, hits);
    check_leptos_local_dialog_state(rel, &content, hits);
}

fn check_handler_state_ctx(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let path_str = rel.to_string_lossy().replace('\\', "/");
    if !path_str.contains("src/transport/http/") {
        return;
    }
    if path_str.contains("src/transport/http/middleware/") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if in_block_comment {
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains("State<Ctx>") && !trimmed.contains("State(ctx)") {
            continue;
        }
        hits.push(Hit {
            rule: "TRANSPORT:23",
            file: path_str.clone(),
            line: line_no + 1,
        });
    }
}

fn check_switchboard_purity(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let mut in_block_comment = false;
    let mut brace_depth: i32 = 0;
    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if in_block_comment {
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        let opens = trimmed.chars().filter(|&c| c == '{').count() as i32;
        let closes = trimmed.chars().filter(|&c| c == '}').count() as i32;
        if brace_depth > 0 {
            brace_depth = (brace_depth + opens - closes).max(0);
            continue;
        }
        let is_mod = trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") || trimmed.starts_with("pub(crate) mod ");
        let is_use = trimmed.starts_with("use ") || trimmed.starts_with("pub use ") || trimmed.starts_with("pub(crate) use ");
        let is_attr = trimmed.starts_with("#[") || trimmed.starts_with("#![");
        let is_extern = trimmed.starts_with("extern crate ");
        if !is_mod && !is_use && !is_attr && !is_extern {
            hit(hits, "DECOMPOSITION:2", rel, line_no + 1);
        }
        if is_use {
            brace_depth = (opens - closes).max(0);
        }
    }
}

fn check_silent_errors(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    for pattern in SILENT_ERROR_PATTERNS {
        if trimmed.contains(pattern) {
            hit(hits, "ERROR:3", rel, line_no + 1);
            break;
        }
    }
}

fn check_wildcard_match_arms(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    for pattern in RESULT_OPTION_MATCH_ARMS {
        if trimmed.starts_with(pattern) {
            hit(hits, "ERROR:4", rel, line_no + 1);
            break;
        }
    }
}

fn check_err_default_arms(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if !(trimmed.starts_with("Err(") || trimmed.starts_with("Err (") || trimmed.starts_with("None =>")) {
        return;
    }
    if !trimmed.contains("=>") {
        return;
    }
    for pattern in ERR_DEFAULT_VALUES {
        if trimmed.contains(pattern) {
            hit(hits, "ERROR:5", rel, line_no + 1);
            break;
        }
    }
}

fn check_empty_err_arms(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let lines: Vec<&str> = content.lines().collect();
    let test_mask = cfg_test_line_mask(content);
    let mut in_block_comment = false;

    for idx in 0..lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if test_mask[idx] {
            continue;
        }
        if in_block_comment || trimmed.starts_with("//") {
            continue;
        }

        let is_err_arm = (trimmed.starts_with("Err(") || trimmed.starts_with("Err (")) && trimmed.contains("=>");
        if !is_err_arm {
            continue;
        }

        if trimmed.contains("=> {}") || trimmed.contains("=> { }") {
            hit(hits, "ERROR:6", rel, idx + 1);
            continue;
        }

        if trimmed.ends_with("=> {") || trimmed.ends_with("=>{") {
            let mut next_idx = idx + 1;
            while next_idx < lines.len() {
                let next_trimmed = lines[next_idx].trim();
                if next_trimmed.is_empty() || next_trimmed.starts_with("//") {
                    next_idx += 1;
                    continue;
                }
                if next_trimmed == "}" {
                    hit(hits, "ERROR:6", rel, idx + 1);
                }
                break;
            }
        }
    }
}

fn check_dead_code_suppression(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if trimmed.contains("#[allow(dead_code") || trimmed.contains("#[allow(unused") || trimmed.contains("#[allow(unreachable_code") {
        hit(hits, "DEAD:8", rel, line_no + 1);
    }
}

fn check_commented_out_code(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    let after_slashes = trimmed.trim_start_matches('/').trim();

    if after_slashes.is_empty() {
        return;
    }

    let code_indicators = [
        "fn ", "let ", "use ", "pub ", "struct ", "enum ", "impl ", "mod ", "match ", "if ", "for ", "while ", "return ", "self.", "Self::", "crate::", "super::", ".await", "async ", "mut ", "const ", "static ",
        "type ", "trait ", "where ", "loop ", "break", "continue",
    ];

    let smells_like_code = code_indicators.iter().any(|indicator| after_slashes.starts_with(indicator));

    let has_code_punctuation =
        after_slashes.ends_with(';') || after_slashes.ends_with('{') || after_slashes.ends_with('}') || after_slashes.ends_with("()") || (after_slashes.contains("::") && after_slashes.contains('('));

    if smells_like_code || has_code_punctuation {
        hit(hits, "DEAD:9", rel, line_no + 1);
    }
}

fn check_unfinished_markers(rel: &Path, line_no: usize, trimmed: &str, is_bin: bool, hits: &mut Vec<Hit>) {
    if is_bin && trimmed.contains("todo!") {
        return;
    }

    let markers = ["todo!(", "unimplemented!(", "unreachable!("];
    for marker in markers {
        if trimmed.contains(marker) {
            hit(hits, "DEAD:10", rel, line_no + 1);
            break;
        }
    }
}

fn check_result_types(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;
    let mut idx = 0usize;

    while idx < lines.len() {
        let trimmed = lines[idx].trim();

        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            idx += 1;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            idx = line_after_cfg_test_item(&lines, idx);
            continue;
        }
        if in_block_comment || trimmed.starts_with("//") {
            idx += 1;
            continue;
        }

        if is_fn_start(trimmed) {
            let start_line = idx + 1;
            let mut sig = trimmed.to_string();
            let mut j = idx + 1;
            while j < lines.len() && !sig.contains('{') && !sig.trim_end().ends_with(';') && sig.len() < 1600 {
                let next = lines[j].trim();
                if !next.starts_with("//") {
                    sig.push(' ');
                    sig.push_str(next);
                }
                j += 1;
            }
            check_return_type(rel, start_line, &sig, hits);
            idx = j;
            continue;
        }

        idx += 1;
    }
}

fn is_fn_start(trimmed: &str) -> bool {
    trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("pub(crate) fn ")
        || trimmed.starts_with("pub(super) fn ")
        || trimmed.starts_with("pub async fn ")
        || trimmed.starts_with("pub(crate) async fn ")
        || trimmed.starts_with("pub(super) async fn ")
        || trimmed.starts_with("async fn ")
}

fn check_return_type(rel: &Path, line_no: usize, sig: &str, hits: &mut Vec<Hit>) {
    let normalized = sig.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.contains("fn main(") {
        return;
    }

    let Some(arrow_idx) = sig.find("->") else {
        return;
    };

    let mut ret = sig[arrow_idx + 2..].trim();
    if let Some(w) = ret.find(" where ") {
        ret = ret[..w].trim();
    }
    if let Some(b) = ret.find('{') {
        ret = ret[..b].trim();
    }
    ret = ret.trim_end_matches(';').trim();

    if ret.starts_with("anyhow::Result") || ret.starts_with("eyre::Result") {
        hit(hits, "TYPE:7", rel, line_no);
        return;
    }

    if !ret.starts_with("Result<") && !ret.starts_with("std::result::Result<") {
        return;
    }

    let prefix_len = if ret.starts_with("std::result::Result<") { "std::result::Result<".len() } else { "Result<".len() };
    let Some(close_idx) = ret.rfind('>') else {
        hit(hits, "TYPE:7", rel, line_no);
        return;
    };

    let inner = &ret[prefix_len..close_idx];
    let Some(comma_idx) = find_top_level_comma(inner) else {
        hit(hits, "TYPE:7", rel, line_no);
        return;
    };

    let err_ty = inner[comma_idx + 1..].trim();
    if is_erased_err_type(err_ty) {
        hit(hits, "TYPE:7", rel, line_no);
    }
}

fn is_erased_err_type(err_ty: &str) -> bool {
    let normalized = err_ty.trim().trim_end_matches(',').trim();

    if normalized.is_empty() || normalized == "()" {
        return true;
    }

    const BANNED_EXACT: &[&str] = &[
        "String",
        "&str",
        "&'static str",
        "str",
        "anyhow::Error",
        "eyre::Report",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "f32",
        "f64",
        "bool",
        "char",
    ];
    if BANNED_EXACT.iter().any(|b| *b == normalized) {
        return true;
    }

    if normalized.contains("dyn ") {
        return true;
    }

    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() == 1 && chars[0].is_ascii_uppercase() {
        return true;
    }

    false
}

fn find_top_level_comma(inner: &str) -> Option<usize> {
    let mut angle = 0usize;
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '<' => angle = angle.saturating_add(1),
            '>' => angle = angle.saturating_sub(1),
            '(' => paren = paren.saturating_add(1),
            ')' => paren = paren.saturating_sub(1),
            '[' => bracket = bracket.saturating_add(1),
            ']' => bracket = bracket.saturating_sub(1),
            '{' => brace = brace.saturating_add(1),
            '}' => brace = brace.saturating_sub(1),
            ',' if angle == 0 && paren == 0 && bracket == 0 && brace == 0 => return Some(idx),
            _ => {}
        }
    }

    None
}

fn layer_for_path(rel: &Path) -> Option<&'static str> {
    let s = rel.to_string_lossy();
    for name in LAYER_NAMES {
        let prefix_slash = format!("{}/", name);
        if s.starts_with(&prefix_slash) {
            return Some(*name);
        }
        let single_file = format!("{}.rs", name);
        if s == *name || s == single_file {
            return Some(*name);
        }
    }
    None
}

fn banned_for_layer(layer: &str) -> &'static [&'static str] {
    match layer {
        "transport" => &["crate::routines", "crate::models", "crate::services", "crate::database", "crate::crank"],
        "flows" => &["crate::models", "crate::services", "crate::database", "crate::transport", "crate::flows"],
        "routines" => &["crate::database", "crate::flows", "crate::transport", "crate::routines", "crate::crank"],
        "models" => &["crate::services", "crate::routines", "crate::flows", "crate::transport", "crate::crank"],
        "services" => &["crate::database", "crate::models", "crate::routines", "crate::flows", "crate::transport", "crate::crank"],
        "database" => &["crate::models", "crate::services", "crate::routines", "crate::flows", "crate::transport", "crate::crank"],
        "structs" => &["crate::transport", "crate::flows", "crate::routines", "crate::models", "crate::services", "crate::database", "crate::crank"],
        _ => &[],
    }
}

fn rule_id_for_layer(layer: &str) -> &'static str {
    match layer {
        "transport" => "LAYER:11",
        "flows" => "LAYER:12",
        "routines" => "LAYER:13",
        "models" => "LAYER:14",
        "services" => "LAYER:15",
        "database" => "LAYER:16",
        "structs" => "LAYER:17",
        _ => "LAYER:0",
    }
}

fn check_layer_imports(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let layer = match layer_for_path(rel) {
        Some(l) => l,
        None => return,
    };
    let banned = banned_for_layer(layer);
    if banned.is_empty() {
        return;
    }
    let rule = rule_id_for_layer(layer);
    let is_structs_schema_exception = layer == "structs";
    let mod_path = module_path_for_rel(rel);

    let lines: Vec<&str> = content.lines().collect();
    let mut idx = 0usize;
    let mut in_block_comment = false;

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            idx += 1;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            idx = line_after_cfg_test_item(&lines, idx);
            continue;
        }
        if in_block_comment {
            idx += 1;
            continue;
        }
        if trimmed.starts_with("//") {
            idx += 1;
            continue;
        }

        let import_rest = if let Some(rest) = trimmed.strip_prefix("use ") {
            Some(rest)
        } else if let Some(rest) = trimmed.strip_prefix("pub use ") {
            Some(rest)
        } else if let Some(rest) = trimmed.strip_prefix("pub(crate) use ") {
            Some(rest)
        } else if let Some(rest) = trimmed.strip_prefix("pub(super) use ") {
            Some(rest)
        } else {
            None
        };

        let rest = match import_rest {
            Some(r) => r,
            None => {
                idx += 1;
                continue;
            }
        };

        let prefix_len = line.len() - rest.len();
        let (stmt_text, line_map, lines_consumed) = collect_use_statement(&lines, idx, prefix_len);

        let leaves = flatten_use_paths(&stmt_text, &line_map, idx + 1);

        for (path, leaf_line) in leaves {
            let resolved = match resolve_to_crate_path(&path, &mod_path) {
                Some(p) => p,
                None => continue, // external import (std::, serde::, leptos::, etc.) — not a layer concern
            };
            if is_structs_schema_exception && import_starts_with(&resolved, "crate::database::schema") {
                continue;
            }
            for ban in banned {
                if import_starts_with(&resolved, ban) {
                    hit(hits, rule, rel, leaf_line);
                    break;
                }
            }
        }

        idx += lines_consumed.max(1);
    }
}

fn collect_use_statement(lines: &[&str], start_idx: usize, prefix_len: usize) -> (String, Vec<usize>, usize) {
    let mut text = String::new();
    let mut line_map: Vec<usize> = Vec::new();
    let mut consumed = 0usize;
    let mut found_semi = false;

    let mut cur = start_idx;
    while cur < lines.len() {
        let line = lines[cur];
        let segment = if cur == start_idx {
            if prefix_len <= line.len() {
                &line[prefix_len..]
            } else {
                ""
            }
        } else {
            line
        };

        let cleaned = strip_inline_comments(segment);

        for ch in cleaned.chars() {
            text.push(ch);
            line_map.push(cur + 1);
            if ch == ';' {
                found_semi = true;
                break;
            }
        }

        consumed += 1;

        if found_semi {
            break;
        }

        text.push('\n');
        line_map.push(cur + 1);
        cur += 1;
    }

    (text, line_map, consumed)
}

fn strip_inline_comments(segment: &str) -> String {
    let chars: Vec<char> = segment.chars().collect();
    let mut out = String::with_capacity(segment.len());
    let mut i = 0usize;
    let mut in_block = false;

    while i < chars.len() {
        if in_block {
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '/' {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            break;
        }
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            in_block = true;
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

fn flatten_use_paths(stmt: &str, line_map: &[usize], fallback_line: usize) -> Vec<(String, usize)> {
    let chars: Vec<char> = stmt.chars().collect();

    let line_at = |ci: usize| -> usize {
        if ci < line_map.len() {
            line_map[ci]
        } else if !line_map.is_empty() {
            line_map[line_map.len() - 1]
        } else {
            fallback_line
        }
    };

    let mut out: Vec<(String, usize)> = Vec::new();
    parse_tree(&chars, 0, "", line_at, fallback_line, &mut out);
    out
}

fn parse_tree<F>(chars: &[char], start: usize, prefix: &str, line_at: F, fallback_line: usize, out: &mut Vec<(String, usize)>) -> usize
where
    F: Fn(usize) -> usize + Copy,
{
    let mut i = start;
    let mut current = String::new();
    let mut current_start: Option<usize> = None;
    let mut depth_in_segment = 0i32;

    while i < chars.len() {
        let ch = chars[i];

        if ch == ';' {
            emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
            return i + 1;
        }

        if ch == '{' {
            let group_prefix = build_prefix(prefix, &current);
            i = parse_group(chars, i + 1, &group_prefix, line_at, fallback_line, out);
            current.clear();
            current_start = None;
            depth_in_segment = 0;
            continue;
        }

        if ch == '}' {
            emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
            return i + 1;
        }

        if ch == ',' && depth_in_segment == 0 {
            emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
            current.clear();
            current_start = None;
            i += 1;
            continue;
        }

        if ch == '(' || ch == '<' || ch == '[' {
            depth_in_segment += 1;
        }
        if ch == ')' || ch == '>' || ch == ']' {
            depth_in_segment -= 1;
        }

        if !ch.is_whitespace() && current_start.is_none() {
            current_start = Some(i);
        }
        current.push(ch);
        i += 1;
    }

    emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
    i
}

fn parse_group<F>(chars: &[char], start: usize, prefix: &str, line_at: F, fallback_line: usize, out: &mut Vec<(String, usize)>) -> usize
where
    F: Fn(usize) -> usize + Copy,
{
    let mut i = start;
    let mut current = String::new();
    let mut current_start: Option<usize> = None;
    let mut depth_in_segment = 0i32;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '}' {
            emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
            return i + 1;
        }

        if ch == '{' {
            let group_prefix = build_prefix(prefix, &current);
            i = parse_group(chars, i + 1, &group_prefix, line_at, fallback_line, out);
            current.clear();
            current_start = None;
            depth_in_segment = 0;
            continue;
        }

        if ch == ',' && depth_in_segment == 0 {
            emit_leaf(prefix, &current, current_start, line_at, fallback_line, out);
            current.clear();
            current_start = None;
            i += 1;
            continue;
        }

        if ch == '(' || ch == '<' || ch == '[' {
            depth_in_segment += 1;
        }
        if ch == ')' || ch == '>' || ch == ']' {
            depth_in_segment -= 1;
        }

        if !ch.is_whitespace() && current_start.is_none() {
            current_start = Some(i);
        }
        current.push(ch);
        i += 1;
    }

    i
}

fn build_prefix(prefix: &str, segment: &str) -> String {
    let trimmed = strip_alias(segment.trim()).trim().trim_end_matches("::").trim();
    if trimmed.is_empty() {
        prefix.to_string()
    } else if prefix.is_empty() {
        trimmed.to_string()
    } else {
        format!("{}::{}", prefix, trimmed)
    }
}

fn emit_leaf<F>(prefix: &str, segment: &str, segment_start: Option<usize>, line_at: F, fallback_line: usize, out: &mut Vec<(String, usize)>)
where
    F: Fn(usize) -> usize,
{
    let cleaned = strip_alias(segment.trim()).trim().to_string();
    if cleaned.is_empty() {
        return;
    }
    if cleaned == "self" {
        let line = segment_start.map(line_at).unwrap_or(fallback_line);
        if !prefix.is_empty() {
            out.push((prefix.to_string(), line));
        }
        return;
    }
    let path = if prefix.is_empty() { cleaned } else { format!("{}::{}", prefix, cleaned) };
    let line = segment_start.map(line_at).unwrap_or(fallback_line);
    out.push((path, line));
}

fn strip_alias(segment: &str) -> &str {
    if let Some(pos) = find_as_keyword(segment) {
        segment[..pos].trim_end()
    } else {
        segment
    }
}

fn find_as_keyword(segment: &str) -> Option<usize> {
    let bytes = segment.as_bytes();
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] == b' ' && bytes[i + 1] == b'a' && bytes[i + 2] == b's' && (bytes[i + 3] == b' ' || bytes[i + 3] == b'\t' || bytes[i + 3] == b'\n') {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Given content lines and the index of a `#[cfg(test)]` attr line, return
/// the line index AFTER the item the attr scopes to. Handles single-line
/// items (`#[cfg(test)] use ...;`) and block items (`mod`, `fn`, `impl`).
/// String-aware (`"..."` with `\` escapes) and comment-aware (`//`, `/* */`).
/// Caller uses the returned index to fast-forward past the test scope and
/// continue scanning production lines below the test mod (closing the
/// "in_test_module flag never reset" escape hatch reported by audit_focus_1).
fn line_after_cfg_test_item(lines: &[&str], attr_idx: usize) -> usize {
    let mut j = attr_idx + 1;
    while j < lines.len() {
        let t = lines[j].trim();
        if t.is_empty() || t.starts_with("//") {
            j += 1;
            continue;
        }
        break;
    }
    if j >= lines.len() {
        return j;
    }
    let item_line = lines[j].trim();
    if item_line.ends_with(';') && !item_line.contains('{') {
        return j + 1;
    }
    let mut depth: i32 = 0;
    let mut entered = false;
    let mut k = j;
    let mut in_block_comment = false;
    while k < lines.len() {
        let line = lines[k];
        let mut chars = line.chars().peekable();
        let mut in_str = false;
        let mut line_comment = false;
        while let Some(c) = chars.next() {
            if line_comment {
                break;
            }
            if in_block_comment {
                if c == '*' {
                    let next = chars.peek();
                    if next == Some(&'/') {
                        chars.next();
                        in_block_comment = false;
                    }
                }
                continue;
            }
            if in_str {
                if c == '\\' {
                    chars.next();
                    continue;
                }
                if c == '"' {
                    in_str = false;
                }
                continue;
            }
            if c == '/' {
                let next = chars.peek();
                if next == Some(&'/') {
                    line_comment = true;
                    continue;
                }
                if next == Some(&'*') {
                    chars.next();
                    in_block_comment = true;
                    continue;
                }
            }
            if c == '"' {
                in_str = true;
                continue;
            }
            if c == '{' {
                depth += 1;
                entered = true;
            } else if c == '}' {
                depth -= 1;
            }
        }
        k += 1;
        if entered && depth == 0 {
            return k;
        }
    }
    k
}

/// Returns one bool per line — true if the line sits inside a `#[cfg(test)]`
/// scope. Used by lint fns that walk lines via `for (line_no, line)` (and
/// thus can't fast-forward via `line_after_cfg_test_item` directly).
fn cfg_test_line_mask(content: &str) -> Vec<bool> {
    let lines: Vec<&str> = content.lines().collect();
    let mut mask = vec![false; lines.len()];
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed != "#[cfg(test)]" {
            idx += 1;
            continue;
        }
        let end = line_after_cfg_test_item(&lines, idx).min(lines.len());
        for k in idx..end {
            mask[k] = true;
        }
        idx = end.max(idx + 1);
    }
    mask
}

/// Compute the canonical `crate::a::b::c` module path for a source file
/// relative to `src/`. `flows/auth/login.rs` → `["crate","flows","auth","login"]`,
/// `flows/auth/mod.rs` → `["crate","flows","auth"]`, `flows.rs` → `["crate","flows"]`.
fn module_path_for_rel(rel: &Path) -> Vec<String> {
    let s = rel.to_string_lossy();
    let stem = s.strip_suffix(".rs").unwrap_or(&s);
    let parts: Vec<&str> = stem.split('/').filter(|p| !p.is_empty()).collect();
    let mut out = vec!["crate".to_string()];
    for (i, p) in parts.iter().enumerate() {
        if i == parts.len() - 1 && *p == "mod" {
            break;
        }
        out.push(p.to_string());
    }
    out
}

/// Resolve a `use`-path leaf to its canonical `crate::…` form so layer bans
/// catch `super::`/`self::` escapes. Returns None for external imports
/// (`std::`, `serde::`, …) — they are not a layer concern.
fn resolve_to_crate_path(path: &str, mod_path: &[String]) -> Option<String> {
    if path == "crate" || path.starts_with("crate::") {
        return Some(path.to_string());
    }
    if path == "self" {
        return Some(mod_path.join("::"));
    }
    let stripped_self = path.strip_prefix("self::");
    match stripped_self {
        Some(rest) => {
            let mut out = mod_path.join("::");
            if !rest.is_empty() {
                out.push_str("::");
                out.push_str(rest);
            }
            return Some(out);
        }
        None => {} // allow: not a self-relative path; fall through to super:: handling
    }
    if !path.starts_with("super::") && path != "super" {
        return None; // external import (`std::`, `serde::`, even `supersize::` — not a super-relative path)
    }
    let mut current: Vec<String> = mod_path.to_vec();
    let mut remaining = path;
    loop {
        let stripped = remaining.strip_prefix("super::");
        match stripped {
            Some(rest) => {
                if current.len() <= 1 {
                    return None; // walked past crate root — invalid path, ignore
                }
                current.pop();
                remaining = rest;
                continue;
            }
            None => {} // allow: no more `super::` prefix; check the trailing `super` form
        }
        if remaining == "super" {
            if current.len() <= 1 {
                return None;
            }
            current.pop();
            return Some(current.join("::"));
        }
        break;
    }
    let mut out = current.join("::");
    if !remaining.is_empty() {
        out.push_str("::");
        out.push_str(remaining);
    }
    Some(out)
}

fn import_starts_with(import_path: &str, ban: &str) -> bool {
    if !import_path.starts_with(ban) {
        return false;
    }
    let rest = &import_path[ban.len()..];
    let next_ch = rest.chars().next();
    match next_ch {
        None => true,
        Some(';') | Some(':') | Some(' ') | Some('{') | Some(',') | Some('}') => true,
        _ => false,
    }
}

fn check_underscore_bind(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if !trimmed.starts_with("let _") {
        return;
    }
    if trimmed.starts_with("let _ =") || trimmed.starts_with("let _:") || trimmed.starts_with("let _ :") {
        return;
    }
    let after = &trimmed["let _".len()..];
    let mut end = 0usize;
    for (i, ch) in after.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = i + ch.len_utf8();
            continue;
        }
        break;
    }
    if end == 0 {
        return;
    }
    let rest = after[end..].trim_start();
    if rest.starts_with(':') {
        return;
    }
    if rest.starts_with('=') {
        hit(hits, "ERROR:19", rel, line_no + 1);
    }
}

fn check_map_err_discard(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    let needle = ".map_err(|_";
    let pos = match trimmed.find(needle) {
        Some(p) => p,
        None => return,
    };
    let after = &trimmed[pos + needle.len()..];
    let mut end = 0usize;
    for (i, ch) in after.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            end = i + ch.len_utf8();
            continue;
        }
        break;
    }
    let next = after[end..].trim_start();
    if next.starts_with('|') {
        hit(hits, "ERROR:20", rel, line_no + 1);
    }
}

fn check_underscore_pattern_bindings(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    let prefixes = ["Err(_", "Ok(_", "Some(_"];
    for pre in &prefixes {
        if !trimmed.starts_with(pre) {
            continue;
        }
        let after = &trimmed[pre.len()..];
        if after.starts_with(')') {
            continue;
        }
        let mut end = 0usize;
        for (i, ch) in after.char_indices() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                end = i + ch.len_utf8();
                continue;
            }
            break;
        }
        if end == 0 {
            continue;
        }
        let rest = after[end..].trim_start();
        if rest.starts_with(") =>") || rest.starts_with(")=>") {
            hit(hits, "ERROR:4", rel, line_no + 1);
            return;
        }
    }
}

fn check_err_arm_handling(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;
    let mut idx = 0usize;

    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            idx += 1;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            idx = line_after_cfg_test_item(&lines, idx);
            continue;
        }
        if in_block_comment || trimmed.starts_with("//") {
            idx += 1;
            continue;
        }

        if let Some(multi_line) = parse_simple_err_arm(trimmed) {
            let body = collect_err_arm_body(&lines, idx, multi_line);
            if !err_body_logs_or_propagates(&body) {
                hit(hits, "ERROR:18", rel, idx + 1);
            }
        }

        idx += 1;
    }
}

fn parse_simple_err_arm(trimmed: &str) -> Option<bool> {
    if !trimmed.starts_with("Err(") {
        return None;
    }
    let after = &trimmed["Err(".len()..];
    let close_paren = after.find(')')?;
    let binding = after[..close_paren].trim();
    if binding == "_" || binding.is_empty() {
        return None;
    }
    if binding.contains("::") || binding.contains('(') || binding.contains(',') || binding.contains(' ') {
        return None;
    }
    let rest = after[close_paren + 1..].trim_start();
    if !rest.starts_with("=>") {
        return None;
    }
    let after_arrow = rest[2..].trim_start();
    Some(after_arrow.starts_with('{'))
}

fn collect_err_arm_body(lines: &[&str], start: usize, multi_line: bool) -> String {
    let first_line = lines[start];
    let arrow_pos = match first_line.find("=>") {
        Some(p) => p,
        None => return first_line.to_string(),
    };
    let first_after = first_line[arrow_pos + 2..].to_string();

    if !multi_line {
        return first_after;
    }

    let mut body = String::new();
    body.push_str(&first_after);
    body.push('\n');
    let mut depth = first_after.chars().filter(|c| *c == '{').count() as i32;
    depth -= first_after.chars().filter(|c| *c == '}').count() as i32;

    if depth <= 0 {
        return body;
    }

    let mut idx = start + 1;
    while idx < lines.len() {
        let line = lines[idx];
        let opens = line.chars().filter(|c| *c == '{').count() as i32;
        let closes = line.chars().filter(|c| *c == '}').count() as i32;
        depth += opens - closes;
        body.push_str(line);
        body.push('\n');
        if depth <= 0 {
            break;
        }
        idx += 1;
    }
    body
}

fn err_body_logs_or_propagates(body: &str) -> bool {
    body.contains("cata_log!(") || body.contains(".log()") || body.contains("return Err(") || body.contains("MeltDown::") || body.contains("Err(") || body.contains("?")
}

fn check_no_comments(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let bytes = content.as_bytes();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut in_str = false;
    let mut in_raw_str = false;
    let mut raw_hashes = 0usize;
    let mut in_block = false;
    let is_generated = is_generated_path(rel);
    let mut seen_code = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if in_block {
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_str {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if in_raw_str {
            if b == b'"' {
                let mut j = i + 1;
                let mut closed = 0usize;
                while j < bytes.len() && bytes[j] == b'#' && closed < raw_hashes {
                    j += 1;
                    closed += 1;
                }
                if closed == raw_hashes {
                    in_raw_str = false;
                    i = j;
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if b == b'r' && i + 1 < bytes.len() && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let mut j = i + 1;
            let mut hashes = 0usize;
            while j < bytes.len() && bytes[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                in_raw_str = true;
                raw_hashes = hashes;
                seen_code = true;
                i = j + 1;
                continue;
            }
        }
        if b == b'b' && i + 1 < bytes.len() && bytes[i + 1] == b'"' {
            in_str = true;
            seen_code = true;
            i += 2;
            continue;
        }
        if b == b'"' {
            in_str = true;
            seen_code = true;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'/' {
                let allow = is_generated && !seen_code;
                if !allow {
                    hit(hits, "DEAD:21", rel, line);
                }
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if next == b'*' {
                let allow = is_generated && !seen_code;
                if !allow {
                    hit(hits, "DEAD:21", rel, line);
                }
                in_block = true;
                i += 2;
                continue;
            }
        }
        if !b.is_ascii_whitespace() {
            seen_code = true;
        }
        i += 1;
    }
}

fn is_generated_path(rel: &Path) -> bool {
    let s = rel.to_string_lossy();
    if s.contains("/generated/") || s.starts_with("generated/") || s.contains("\\generated\\") || s.starts_with("generated\\") {
        return true;
    }
    matches!(s.as_ref(), "database/schema.rs")
}

fn is_data_definition_allowed_file(rel: &Path) -> bool {
    let s = rel.to_string_lossy();
    if s.starts_with("structs/") || s.starts_with("structs\\") {
        return true;
    }
    if s.starts_with("testing/") || s.starts_with("testing\\") {
        return true;
    }
    matches!(s.as_ref(), "meltdown.rs" | "ctx.rs" | "crank.rs" | "cata_log.rs" | "bootstrap.rs" | "lib.rs" | "main.rs" | "database/schema.rs")
}

fn is_struct_or_enum_def(trimmed: &str) -> bool {
    let mut s = trimmed;
    for prefix in ["pub(crate) ", "pub(super) ", "pub(self) ", "pub "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest;
            break;
        }
    }

    let after = if let Some(rest) = s.strip_prefix("struct ") {
        rest
    } else if let Some(rest) = s.strip_prefix("enum ") {
        rest
    } else {
        return false;
    };

    after.chars().next().map_or(false, |c| c.is_ascii_alphabetic() || c == '_')
}

fn path_under(rel: &Path, segment: &str) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    s.starts_with(segment)
}

fn skip_line_for_leptos_scan(trimmed: &str, in_block_comment: &mut bool) -> bool {
    if trimmed.contains("/*") {
        *in_block_comment = true;
    }
    if trimmed.contains("*/") {
        *in_block_comment = false;
        return true;
    }
    if *in_block_comment {
        return true;
    }
    if trimmed.starts_with("//") {
        return true;
    }
    false
}

fn check_leptos_inline_style(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if leptos_line_has_inline_style(trimmed) {
            hit(hits, "LEPTOS:1", rel, line_no + 1);
        }
    }
}

fn leptos_line_has_inline_style(line: &str) -> bool {
    let bytes = line.as_bytes();
    let needles: [&[u8]; 2] = [b"style=\"", b"style=format!"];
    for needle in needles {
        let mut start = 0usize;
        while start + needle.len() <= bytes.len() {
            let slice = &bytes[start..start + needle.len()];
            if slice == needle {
                if start == 0 {
                    return true;
                }
                let prev = bytes[start - 1];
                let prev_is_word = prev.is_ascii_alphanumeric() || prev == b'_';
                if !prev_is_word {
                    return true;
                }
            }
            start += 1;
        }
    }
    false
}

fn check_leptos_hex_colors(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") && !path_under(rel, "structs/") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if leptos_line_has_color_literal(raw) {
            hit(hits, "LEPTOS:2", rel, line_no + 1);
        }
    }
}

fn leptos_line_has_color_literal(line: &str) -> bool {
    let funcs = ["rgb(", "rgba(", "hsl(", "hsla("];
    for f in funcs {
        if line.contains(f) {
            return true;
        }
    }
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && (bytes[j] as char).is_ascii_hexdigit() {
            j += 1;
        }
        let hex_len = j - (i + 1);
        if (3..=8).contains(&hex_len) {
            let after_is_word = j < bytes.len() && ((bytes[j] as char).is_ascii_alphanumeric() || bytes[j] == b'_');
            if !after_is_word {
                return true;
            }
        }
        i = j.max(i + 1);
    }
    false
}

fn check_leptos_px_units(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if raw.contains("@media") || raw.contains("0.0625rem") {
            continue;
        }
        if leptos_line_has_px_unit(raw) {
            hit(hits, "LEPTOS:3", rel, line_no + 1);
        }
    }
}

fn leptos_line_has_px_unit(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !(bytes[i] as char).is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'.' && j + 1 < bytes.len() && (bytes[j + 1] as char).is_ascii_digit() {
            j += 1;
            while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                j += 1;
            }
        }
        if j + 1 < bytes.len() && bytes[j] == b'p' && bytes[j + 1] == b'x' {
            let after = j + 2;
            let next_is_word = after < bytes.len() && ((bytes[after] as char).is_ascii_alphanumeric() || bytes[after] == b'_');
            if !next_is_word {
                return true;
            }
        }
        i = j.max(i + 1);
    }
    false
}

fn check_leptos_page_shell_required(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/pages/") {
        return;
    }
    let file_name = rel.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    if file_name == "mod.rs" {
        return;
    }
    let mut in_block_comment = false;
    let mut found_page_fn: Option<usize> = None;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if line_declares_page_component(trimmed) {
            found_page_fn = Some(line_no + 1);
            break;
        }
    }
    let page_fn_line = match found_page_fn {
        Some(n) => n,
        None => return,
    };
    if !content.contains("<PageShell") {
        hit(hits, "LEPTOS:4", rel, page_fn_line);
    }
}

fn line_declares_page_component(trimmed: &str) -> bool {
    let prefixes = ["pub fn ", "pub async fn ", "pub(crate) fn ", "pub(crate) async fn "];
    let mut after: Option<&str> = None;
    for p in prefixes {
        if let Some(rest) = trimmed.strip_prefix(p) {
            after = Some(rest);
            break;
        }
    }
    let rest = match after {
        Some(r) => r,
        None => return false,
    };
    let paren_idx = match rest.find('(') {
        Some(p) => p,
        None => return false,
    };
    let name = &rest[..paren_idx];
    if !name.ends_with("Page") {
        return false;
    }
    if name.len() < 5 {
        return false;
    }
    let first = match name.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_uppercase() {
        return false;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    let after_paren = rest[paren_idx..].trim_start_matches('(');
    let close = match after_paren.find(')') {
        Some(c) => c,
        None => return false,
    };
    let between = after_paren[..close].trim();
    if !between.is_empty() {
        return false;
    }
    let tail = &after_paren[close + 1..];
    tail.contains("-> impl IntoView") || tail.contains("->impl IntoView")
}

fn check_leptos_hardcoded_route_path(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if raw.contains("RouteName::") {
            continue;
        }
        if leptos_line_has_hardcoded_route(raw) {
            hit(hits, "LEPTOS:5", rel, line_no + 1);
        }
    }
}

fn leptos_line_has_hardcoded_route(line: &str) -> bool {
    if literal_starts_with_slash_in_a_href(line, "<a href=\"") {
        return true;
    }
    if literal_starts_with_slash_in_a_href(line, "<A href=\"") {
        return true;
    }
    if nav_first_arg_is_hardcoded_path(line) {
        return true;
    }
    false
}

fn literal_starts_with_slash_in_a_href(line: &str, needle: &str) -> bool {
    let mut start = 0usize;
    while let Some(idx) = line[start..].find(needle) {
        let after = &line[start + idx + needle.len()..];
        if literal_is_disallowed_route(after) {
            return true;
        }
        start += idx + needle.len();
    }
    false
}

fn literal_is_disallowed_route(rest_of_line: &str) -> bool {
    let close = match rest_of_line.find('"') {
        Some(c) => c,
        None => return false,
    };
    let value = &rest_of_line[..close];
    if value.is_empty() {
        return false;
    }
    if value == "#" {
        return false;
    }
    if value.starts_with("mailto:") || value.starts_with("tel:") {
        return false;
    }
    if value.starts_with("https://") || value.starts_with("http://") {
        return false;
    }
    if value.starts_with("//") {
        return false;
    }
    value.starts_with('/')
}

fn nav_first_arg_is_hardcoded_path(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'n' {
            i += 1;
            continue;
        }
        if i + 3 > bytes.len() || &bytes[i..i + 3] != b"nav" {
            i += 1;
            continue;
        }
        if i > 0 {
            let prev = bytes[i - 1];
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                i += 1;
                continue;
            }
        }
        let mut j = i + 3;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'(' {
            i += 1;
            continue;
        }
        j += 1;
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'"' {
            i = j;
            continue;
        }
        let after = &line[j + 1..];
        if literal_is_disallowed_route(after) {
            return true;
        }
        i = j + 1;
    }
    false
}

fn check_leptos_optimistic_update_in_custom(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/pages/") && !path_under(rel, "transport/leptos/components/") {
        return;
    }
    if path_contains_segment(rel, "generated") {
        return;
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        if !line.contains("pending.set(true)") {
            idx += 1;
            continue;
        }
        let mut spawn_idx: Option<usize> = None;
        let mut k = idx + 1;
        while k < lines.len() && k < idx + 80 {
            if lines[k].contains("spawn_local(") {
                spawn_idx = Some(k);
                break;
            }
            k += 1;
        }
        let spawn_line = match spawn_idx {
            Some(n) => n,
            None => {
                idx += 1;
                continue;
            }
        };
        for between in (idx + 1)..spawn_line {
            let raw = lines[between];
            let trimmed = raw.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            if !(trimmed.contains(".set(") || trimmed.contains(".update(")) {
                continue;
            }
            if leptos_line_smells_like_collection_mutation(raw) {
                hit(hits, "LEPTOS:6", rel, between + 1);
            }
        }
        idx = spawn_line + 1;
    }
}

fn leptos_line_smells_like_collection_mutation(line: &str) -> bool {
    if line.contains("vec![") || line.contains("Vec::") || line.contains("HashMap::") || line.contains("BTreeMap::") {
        return true;
    }
    if line.contains(".push(") || line.contains(".extend(") || line.contains(".insert(") || line.contains(".remove(") || line.contains(".retain(") || line.contains(".clear(") {
        return true;
    }
    false
}

fn check_leptos_loading_spinner_after_first_load(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") {
        return;
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;
    for (line_no, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if !leptos_line_has_loading_literal(raw) {
            continue;
        }
        if leptos_loading_in_allowed_context(&lines, line_no) {
            continue;
        }
        hit(hits, "LEPTOS:7", rel, line_no + 1);
    }
}

fn leptos_line_has_loading_literal(line: &str) -> bool {
    let needles = ["\"Loading...\"", "\"Loading…\"", "\"loading...\"", "\"loading…\""];
    for n in needles {
        if line.contains(n) {
            return true;
        }
    }
    false
}

fn leptos_loading_in_allowed_context(lines: &[&str], line_no: usize) -> bool {
    let start = line_no.saturating_sub(8);
    for k in start..=line_no {
        let upper = lines[k];
        if upper.contains("None =>") || upper.contains("None=>") {
            return true;
        }
        if upper.contains("Suspense") && upper.contains("fallback") {
            return true;
        }
        if upper.contains("<Suspense") {
            return true;
        }
        if upper.contains("fallback=") {
            return true;
        }
    }
    false
}

fn check_leptos_local_list_state(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/") {
        return;
    }
    let path_str = rel.to_string_lossy().replace('\\', "/");
    if path_str.ends_with("transport/leptos/signals/url.rs") {
        return;
    }
    if path_contains_segment(rel, "generated") {
        return;
    }
    let mut in_block_comment = false;
    for (line_no, raw) in content.lines().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if raw.contains("ListQuery::default()") {
            hit(hits, "LEPTOS:8", rel, line_no + 1);
        }
    }
}

fn check_leptos_local_dialog_state(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if !path_under(rel, "transport/leptos/pages/") && !path_under(rel, "transport/leptos/components/") {
        return;
    }
    if path_contains_segment(rel, "generated") {
        return;
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;
    for (line_no, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if skip_line_for_leptos_scan(trimmed, &mut in_block_comment) {
            continue;
        }
        if !raw.contains("RwSignal::new(false)") {
            continue;
        }
        if line_window_has_dialog_token(&lines, line_no, 5) {
            hit(hits, "LEPTOS:9", rel, line_no + 1);
        }
    }
}

fn line_window_has_dialog_token(lines: &[&str], line_no: usize, window: usize) -> bool {
    let start = line_no.saturating_sub(window);
    let end = (line_no + window + 1).min(lines.len());
    for k in start..end {
        if line_has_dialog_identifier(lines[k]) {
            return true;
        }
    }
    false
}

fn line_has_dialog_identifier(line: &str) -> bool {
    let tokens = ["dialog", "drawer", "modal", "popup"];
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for tok in tokens {
        let needle = tok.as_bytes();
        let mut start = 0usize;
        while start + needle.len() <= bytes.len() {
            if &bytes[start..start + needle.len()] == needle {
                let prev_ok = if start == 0 {
                    true
                } else {
                    let p = bytes[start - 1];
                    !(p.is_ascii_alphanumeric() || p == b'_')
                };
                let after = start + needle.len();
                let next_ok = if after >= bytes.len() {
                    true
                } else {
                    let n = bytes[after];
                    !(n.is_ascii_lowercase())
                };
                if prev_ok && next_ok {
                    return true;
                }
            }
            start += 1;
        }
    }
    false
}

fn scan_module_scss(manifest_dir: &Path, path: &Path, hits: &mut Vec<Hit>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let src_dir = manifest_dir.join("src");
    let rel = path.strip_prefix(&src_dir).unwrap_or(path);
    check_scss_form_control_font_size(rel, &content, hits);
}

// LEPTOS:10 — form-control font-size must reference `var(--app-fs-*)` or
// `inherit` so the rem-scaled root scales it on 4K. We track brace depth to
// know what selector currently owns the open block, and flag any
// `font-size:` declaration whose enclosing selector targets a form control.
fn check_scss_form_control_font_size(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    // Strip block + line comments first so we don't mis-parse a brace inside
    // a comment. Keep line numbers intact by replacing comment bodies with
    // spaces (and preserving newlines).
    let cleaned = strip_scss_comments(content);

    // Stack of bool flags: `true` = the enclosing block targets a form control.
    let mut form_control_stack: Vec<bool> = Vec::new();
    // Selector accumulator — text from the previous `{` (or file start) up to
    // the next `{`. Reset on every `{` / `}`.
    let mut selector_buf = String::new();

    for (line_no, raw) in cleaned.lines().enumerate() {
        let mut chars = raw.chars().peekable();
        let mut col_buf = String::new();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    let sel_text = format!("{}{}", selector_buf, col_buf).trim().to_string();
                    let is_form = scss_selector_targets_form_control(&sel_text);
                    form_control_stack.push(is_form);
                    selector_buf.clear();
                    col_buf.clear();
                }
                '}' => {
                    form_control_stack.pop();
                    selector_buf.clear();
                    col_buf.clear();
                }
                ';' => {
                    let decl = format!("{}{}", selector_buf, col_buf);
                    let in_form_control_block = form_control_stack.last().copied().unwrap_or(false);
                    if in_form_control_block && scss_decl_violates_font_size(&decl) {
                        hit(hits, "LEPTOS:10", rel, line_no + 1);
                    }
                    selector_buf.clear();
                    col_buf.clear();
                }
                _ => col_buf.push(c),
            }
        }
        // End of line — carry remaining buffer into the running selector
        // accumulator (selectors can wrap across lines before the `{`).
        selector_buf.push_str(&col_buf);
        selector_buf.push('\n');
    }
}

fn strip_scss_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let mut in_block = false;
    let mut in_line = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_block {
            if c == '*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                out.push(' ');
                out.push(' ');
                i += 2;
                in_block = false;
                continue;
            }
            if c == '\n' {
                out.push('\n');
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }
        if in_line {
            if c == '\n' {
                out.push('\n');
                in_line = false;
            } else {
                out.push(' ');
            }
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_block = true;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            in_line = true;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn scss_selector_targets_form_control(selector: &str) -> bool {
    // A selector list is comma-separated. Each segment is e.g.
    // `.foo button:hover`, `& > input`, `select.bar`. We flag the block if
    // ANY segment's last simple-selector identifier is a form-control tag.
    for seg in selector.split(',') {
        let cleaned = seg.trim();
        if cleaned.is_empty() {
            continue;
        }
        let last_simple = cleaned.split_whitespace().last().unwrap_or("");
        if simple_selector_is_form_control(last_simple) {
            return true;
        }
    }
    false
}

fn simple_selector_is_form_control(simple: &str) -> bool {
    // Strip leading combinator chars (`&`, `>`, `+`, `~`).
    let trimmed = simple.trim_start_matches(|c: char| matches!(c, '&' | '>' | '+' | '~'));
    // Take the leading tag-name run (alphabetic, then alphanumeric/-).
    let mut end = 0usize;
    for (i, ch) in trimmed.char_indices() {
        if i == 0 {
            if !ch.is_ascii_alphabetic() {
                return false;
            }
        } else if !(ch.is_ascii_alphanumeric() || ch == '-') {
            end = i;
            break;
        }
        end = i + ch.len_utf8();
    }
    let tag = &trimmed[..end];
    matches!(tag, "input" | "select" | "textarea" | "button")
}

fn scss_decl_violates_font_size(decl: &str) -> bool {
    let trimmed = decl.trim();
    let lower = trimmed.to_ascii_lowercase();
    // Match `font-size` or shorthand `font:` (when shorthand carries a size).
    // We only flag explicit `font-size:` here — `font: inherit` in base.scss
    // is the canonical safe value, but inside a module.scss the explicit
    // `font-size:` declaration is what we lint against.
    let prop_idx = match lower.find("font-size") {
        Some(idx) => idx,
        None => return false,
    };
    // Ensure `font-size` is not part of a longer identifier (e.g. `--font-size-foo`).
    if prop_idx > 0 {
        let prev = lower.as_bytes()[prop_idx - 1] as char;
        if prev.is_ascii_alphanumeric() || prev == '_' || prev == '-' {
            return false;
        }
    }
    let after = &lower[prop_idx + "font-size".len()..];
    let after_trimmed = after.trim_start();
    if !after_trimmed.starts_with(':') {
        return false;
    }
    let value = after_trimmed[1..].trim();
    if value.is_empty() {
        return false;
    }
    if value.contains("var(--app-fs-") {
        return false;
    }
    if value.starts_with("inherit") {
        return false;
    }
    true
}

fn path_contains_segment(rel: &Path, segment: &str) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    let needle_mid = format!("/{}/", segment);
    let needle_start = format!("{}/", segment);
    s.contains(&needle_mid) || s.starts_with(&needle_start)
}

fn check_inline_data_definitions(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if is_data_definition_allowed_file(rel) {
        return;
    }

    let test_mask = cfg_test_line_mask(content);
    let mut in_block_comment = false;
    let mut in_macro_rules = false;
    let mut macro_brace_depth: i32 = 0;

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if test_mask[idx] {
            continue;
        }

        if in_macro_rules {
            macro_brace_depth += trimmed.chars().filter(|&c| c == '{').count() as i32;
            macro_brace_depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            if macro_brace_depth <= 0 {
                in_macro_rules = false;
            }
            continue;
        }
        if !in_block_comment && !trimmed.starts_with("//") && trimmed.starts_with("macro_rules!") {
            in_macro_rules = true;
            macro_brace_depth = trimmed.chars().filter(|&c| c == '{').count() as i32;
            macro_brace_depth -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            if macro_brace_depth <= 0 {
                in_macro_rules = false;
            }
            continue;
        }

        if in_block_comment {
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }

        if is_struct_or_enum_def(trimmed) {
            hit(hits, "STRUCTS:22", rel, idx + 1);
        }
    }
}
