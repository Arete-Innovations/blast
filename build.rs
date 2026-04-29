// Lint enforcer for the blast crate. Scans `src/**.rs` and panics the build
// on any rule violation. Forked from buildahead with local patches.
//
// CATEGORIES (panic-on-hit):
//   DECOMPOSITION: file size + lib.rs/mod.rs purity.
//   ERROR:         honest failure handling (propagate or crash, no silent eat).
//   TYPE:          named error types only — no anyhow/eyre/Box<dyn>/String.
//   DEAD:          no #[allow(dead_code)], no commented-out code, no todo!().
//   DANGER:        unsafe / std::process::exit / non-test panic! sites.
//
// ESCAPE VALVE:
//   Append `// allow: <reason>` to a line to exempt it from ERROR:3/4/5/6 and
//   DANGER:13. This forces the author to explicitly justify the deviation
//   instead of either rewriting the code into uglier shapes or silently
//   bypassing the rule via tricks like `drop(e); false` or `match { ...
//   Err(_) => panic!(...) }`.
//
//   Examples:
//     let _ = result; // allow: best-effort kill, server may already be dead
//     Err(_e) => false, // allow: env var read, default is documented elsewhere
//     panic!("regex compile failed"); // allow: constant pattern, infallible
//
//   The marker MUST appear on the same physical line as the offending token.
//   No bulk allow-files, no #[allow(...)] attributes — both have proven to
//   accumulate stale exemptions that nobody ever revisits.
//
// HISTORY OF LOCAL PATCHES:
//   - relaxed `while let Some(` / `while let Ok(` — these iterate, they do not swallow errors; the let-else trick people were forced into was uglier and longer than the original.
//   - added `// allow: <reason>` escape valve (was: no escape, leading to `drop(e); false`, lying default arms, and `match Regex::new(...) { Ok(r) => r, Err(_) => panic!(...) }` workarounds).
//   - added DANGER category for `unsafe`, `std::process::exit(`, and non-test `panic!(` sites. Forward-looking — current codebase has zero `unsafe` and zero raw `process::exit`; the panics in `governor/rules/*` all
//     carry the new `// allow:` marker.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

const MAX_LOC: usize = 800;

// `// allow: <reason>` on a line silences ERROR:3/4/5/6 and DANGER:13 hits
// for that line. The reason text is mandatory; an empty `// allow:` does NOT
// match (we require at least one non-whitespace char after the colon).
const ALLOW_MARKER: &str = "// allow:";

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
    // NOTE: `while let Some(` / `while let Ok(` are intentionally NOT here.
    // They are the standard way to drain an iterator/channel/reader. Banning
    // them forced rewrites into `loop { let Some(x) = it.next() else {
    // break }; ... }` — strictly more code, identical semantics, no safety
    // gain.
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
    if rule.starts_with("DANGER:") {
        return "DANGER";
    }
    "OTHER"
}

fn category_spirit(cat: &str) -> &'static str {
    match cat {
        "DECOMPOSITION" => "every file has one job. if it outgrew that job, split it.",
        "ERROR" => "propagate or crash. there is no third option. if an operation failed, the caller must know.",
        "TYPE" => "the type signature is the contract. if the error type is erased, the contract is worthless.",
        "DEAD" => "code that isn't serving the program right now is noise. noise misleads. delete it.",
        "DANGER" => "abort, unsafe, raw exit. each one bypasses the language's safety story. if you really need it, justify it on the line.",
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
            "    there are exactly two honest responses to failure: tell the caller, or crash.\n",
            "    if a single site is genuinely safe, append `// allow: <reason>` to opt out.",
        ),
        "ERROR:4" => concat!(
            "Ok(_), Err(_), Some(_), None => all throw away the value you're matching on.\n",
            "    bind it: Ok(val), Err(err), Some(val). if you don't need the value, you don't need the match.",
        ),
        "ERROR:5" => concat!(
            "returning Vec::new(), 0, None, false, or Default::default() from an error arm is lying.\n",
            "    the operation failed. say so. return Err and let the caller decide what 'empty' means.\n",
            "    if the fallback is genuinely the right answer, append `// allow: <reason>` and explain.",
        ),
        "ERROR:6" => concat!(
            "an Err arm that does nothing is the #1 source of 'it silently stopped working' bugs.\n",
            "    you received an error, inspected it, and chose to pretend it didn't happen.\n",
            "    if you really want to discard, append `// allow: <reason>` to acknowledge the trade.",
        ),
        "TYPE:7" => concat!(
            "Result<T> with no error type is anyhow in disguise. anyhow/eyre erase what went wrong.\n",
            "    use Result<T, YourError> where YourError is a named enum. no String, no Box<dyn>, no eyre.",
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
        "DANGER:11" => concat!(
            "unsafe blocks/fns dodge the borrow checker. for blast (a dev-time CLI) we have zero need.\n",
            "    if you really must, append `// allow: <reason>` and document the invariants you're upholding.",
        ),
        "DANGER:12" => concat!(
            "std::process::exit() bypasses Drop, leaks tempfiles, kills loggers mid-flush.\n",
            "    return from main with a Result instead. only the bin entry point may exit.",
        ),
        "DANGER:13" => concat!(
            "panic!() in non-test, non-bin code is a crash you didn't write a recovery story for.\n",
            "    either return Err with a real error variant, or append `// allow: <reason>` to acknowledge\n",
            "    that this site is genuinely infallible (e.g. compile-time-constant regex).",
        ),
        _ => "",
    }
}

const CATEGORY_ORDER: &[&str] = &["DECOMPOSITION", "ERROR", "TYPE", "DEAD", "DANGER"];

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
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));

    let templates_canonical = manifest_dir.join("templates").join("canonical");
    if templates_canonical.is_dir() {
        clean_template_artifacts(&templates_canonical);
        emit_rerun_for_tree(&templates_canonical);
    }

    let src_dir = manifest_dir.join("src");
    let mut hits: Vec<Hit> = Vec::new();

    if src_dir.is_dir() {
        scan_dir(&manifest_dir, &src_dir, &mut hits);
    }

    if !hits.is_empty() {
        panic!("\n{}", format_report(&hits));
    }
}

const TEMPLATE_ARTIFACT_DIRS: &[&str] = &["target", "node_modules", "dist", ".vite", ".turbo", ".next", ".git"];

fn emit_rerun_for_tree(root: &Path) {
    println!("cargo:rerun-if-changed={}", root.display());
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_for_tree(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn clean_template_artifacts(root: &Path) {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if TEMPLATE_ARTIFACT_DIRS.contains(&name) {
            println!("cargo:warning=blast: removing template artifact dir {}", path.display());
            if let Err(e) = fs::remove_dir_all(&path) {
                println!("cargo:warning=blast: failed to remove {}: {}", path.display(), e);
            }
        } else {
            clean_template_artifacts(&path);
        }
    }
}

fn hit(hits: &mut Vec<Hit>, rule: &'static str, file: &Path, line: usize) {
    hits.push(Hit {
        rule,
        file: file.to_string_lossy().to_string(),
        line,
    });
}

// Returns true if `line` carries an `// allow: <non-empty-reason>` marker.
// Empty reasons (e.g. `// allow:` or `// allow:    `) do NOT match — the
// rule is "explain or fix", not "incantate to silence".
fn line_has_allow(line: &str) -> bool {
    let Some(idx) = line.find(ALLOW_MARKER) else {
        return false;
    };
    let after = &line[idx + ALLOW_MARKER.len()..];
    after.trim().chars().next().is_some()
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
        }
    }
}

fn strip_raw_strings(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_raw = false;
    for line in content.lines() {
        if in_raw {
            if line.contains("\"#") {
                in_raw = false;
            }
            out.push('\n');
            continue;
        }
        if let Some(open_idx) = line.find("r#\"") {
            let after_open = &line[open_idx + 3..];
            if after_open.contains("\"#") {
                out.push('\n');
                continue;
            }
            in_raw = true;
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn scan_file(manifest_dir: &Path, path: &Path, hits: &mut Vec<Hit>) {
    let raw_content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let content = strip_raw_strings(&raw_content);

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

    let mut in_block_comment = false;
    let mut in_test_module = false;
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
        if trimmed == "#[cfg(test)]" {
            in_test_module = true;
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

        if in_block_comment || in_test_module {
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
        check_unsafe(rel, line_no, trimmed, hits);
        check_process_exit(rel, line_no, trimmed, is_bin, hits);
        check_panic(rel, line_no, trimmed, is_bin, hits);
    }

    check_empty_err_arms(rel, &content, hits);
    check_result_types(rel, &content, hits);
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
    if line_has_allow(trimmed) {
        return;
    }
    for pattern in SILENT_ERROR_PATTERNS {
        if trimmed.contains(pattern) {
            hit(hits, "ERROR:3", rel, line_no + 1);
            break;
        }
    }
}

fn check_wildcard_match_arms(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if line_has_allow(trimmed) {
        return;
    }
    for pattern in RESULT_OPTION_MATCH_ARMS {
        if trimmed.starts_with(pattern) {
            hit(hits, "ERROR:4", rel, line_no + 1);
            break;
        }
    }
}

fn check_err_default_arms(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if line_has_allow(trimmed) {
        return;
    }
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

// DANGER:11 — `unsafe ` blocks/fns. We allow `unsafe impl Send for ...` etc.
// to also be caught (any usage). Marker exemption permitted.
fn check_unsafe(rel: &Path, line_no: usize, trimmed: &str, hits: &mut Vec<Hit>) {
    if line_has_allow(trimmed) {
        return;
    }
    // Match keyword usage, not occurrences inside identifiers like
    // `unsafely_named_fn` — require trailing space, brace, or impl-keyword.
    let starts = trimmed.starts_with("unsafe ")
        || trimmed.starts_with("unsafe{")
        || trimmed.starts_with("pub unsafe ")
        || trimmed.starts_with("pub(crate) unsafe ")
        || trimmed.contains(" unsafe ")
        || trimmed.contains(" unsafe{")
        || trimmed.contains("=unsafe ")
        || trimmed.contains("=unsafe{");
    if starts {
        hit(hits, "DANGER:11", rel, line_no + 1);
    }
}

// DANGER:12 — raw process exit. Only main.rs is exempt; everywhere else must
// return errors via the normal channel so Drop runs. No marker exemption —
// there is no "ok this time" version of bypassing the runtime.
fn check_process_exit(rel: &Path, line_no: usize, trimmed: &str, is_main: bool, hits: &mut Vec<Hit>) {
    if is_main {
        return;
    }
    let hits_pat = trimmed.contains("std::process::exit(") || trimmed.contains("process::exit(") || trimmed.contains("libc::exit(") || trimmed.contains("libc::_exit(");
    if hits_pat {
        hit(hits, "DANGER:12", rel, line_no + 1);
    }
}

// DANGER:13 — non-test, non-bin `panic!(`. We rely on the `in_test_module`
// guard upstream (callers skip this for cfg(test) blocks) and on the
// is_bin flag. Marker exemption permitted for genuinely-infallible sites
// like compile-time-constant regex compilation.
fn check_panic(rel: &Path, line_no: usize, trimmed: &str, is_bin: bool, hits: &mut Vec<Hit>) {
    if is_bin {
        return;
    }
    if line_has_allow(trimmed) {
        return;
    }
    if trimmed.contains("panic!(") {
        hit(hits, "DANGER:13", rel, line_no + 1);
    }
}

fn check_empty_err_arms(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;
    let mut in_test_module = false;

    for idx in 0..lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.contains("/*") {
            in_block_comment = true;
        }
        if trimmed.contains("*/") {
            in_block_comment = false;
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            in_test_module = true;
            continue;
        }
        if in_block_comment || in_test_module || trimmed.starts_with("//") {
            continue;
        }

        let is_err_arm = (trimmed.starts_with("Err(") || trimmed.starts_with("Err (")) && trimmed.contains("=>");
        if !is_err_arm {
            continue;
        }
        if line_has_allow(trimmed) {
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
    let mut in_test_module = false;
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
            in_test_module = true;
            idx += 1;
            continue;
        }
        if in_block_comment || in_test_module || trimmed.starts_with("//") {
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
    if err_ty.contains("Infallible") {
        return;
    }
    if is_erased_err_ty(err_ty) {
        hit(hits, "TYPE:7", rel, line_no);
    }
}

fn is_erased_err_ty(err_ty: &str) -> bool {
    let bad_substrings = ["Box<dyn", "anyhow::", "eyre::"];
    if bad_substrings.iter().any(|b| err_ty.contains(b)) {
        return true;
    }
    matches!(err_ty, "String" | "&str" | "&'static str" | "()")
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
