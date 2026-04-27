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
        _ => "",
    }
}

const CATEGORY_ORDER: &[&str] = &["DECOMPOSITION", "LAYER", "STRUCTS", "ERROR", "TYPE", "DEAD"];

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
    let src_dir = manifest_dir.join("src");
    let mut hits: Vec<Hit> = Vec::new();

    if src_dir.is_dir() {
        scan_dir(&manifest_dir, &src_dir, &mut hits);
    }

    if !hits.is_empty() {
        panic!("\n{}", format_report(&hits));
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
        }
    }
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

    let lines: Vec<&str> = content.lines().collect();
    let mut idx = 0usize;
    let mut in_block_comment = false;
    let mut in_test_module = false;

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
            in_test_module = true;
            idx += 1;
            continue;
        }
        if in_block_comment || in_test_module {
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
            if !path.starts_with("crate::") && path != "crate" {
                continue;
            }
            if is_structs_schema_exception && import_starts_with(&path, "crate::database::schema") {
                continue;
            }
            for ban in banned {
                if import_starts_with(&path, ban) {
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
    s.contains("/generated/") || s.starts_with("generated/") || s.contains("\\generated\\") || s.starts_with("generated\\")
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

fn check_inline_data_definitions(rel: &Path, content: &str, hits: &mut Vec<Hit>) {
    if is_data_definition_allowed_file(rel) {
        return;
    }

    let mut in_block_comment = false;
    let mut in_test_module = false;
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
            continue;
        }

        if is_struct_or_enum_def(trimmed) {
            hit(hits, "STRUCTS:22", rel, idx + 1);
        }
    }
}
