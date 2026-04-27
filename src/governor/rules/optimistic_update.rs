use crate::governor::rules::helpers::{extension_is, path_contains, snippet_of};
use crate::governor::rules::traits::FileRule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

lazy_static! {
    /// Captures `const xxxFn = useUpdateFoo()` / `useCreateFoo()` /
    /// `useDeleteFoo()`. Group 1 = local binding name, group 2 = composable.
    static ref MUTATION_BIND_RE: Regex = match Regex::new(
        r"\b(?:const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)\s*=\s*(use(?:Update|Create|Delete)[A-Z][A-Za-z0-9_$]*)\s*\("
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("OptimisticUpdate mutation regex failed to compile"), // allow: const pattern, infallible
    };
    /// Local-state mutation patterns: `x.value = ...`, `x.value.push(...)`,
    /// `state.x = ...`.
    static ref LOCAL_MUTATION_RE: Regex = match Regex::new(
        r"\b[A-Za-z_$][A-Za-z0-9_$]*\.value\s*(?:=|\.push\b|\.splice\b)|\bstate\.[A-Za-z_$][A-Za-z0-9_$]*\s*="
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("OptimisticUpdate local-mut regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct OptimisticUpdateInCustom;

impl OptimisticUpdateInCustom {
    pub fn new() -> Self {
        Self
    }
}

fn is_user_file(file: &Path) -> bool {
    !path_contains(file, "/generated/")
}

/// Walk the file looking for: a mutation hook call, then a function/handler
/// block that invokes the bound mutation AND mutates local state before any
/// `await`. The heuristic is: scan each function body that calls one of the
/// known mutation bindings; flag if a local-mutation pattern appears in the
/// same body before the first `await`.
fn scan_for_optimistic(contents: &str) -> Vec<(usize, String)> {
    let mut bindings: Vec<String> = Vec::new();
    for caps in MUTATION_BIND_RE.captures_iter(contents) {
        match caps.get(1) {
            Some(m) => bindings.push(m.as_str().to_string()),
            None => continue,
        }
    }
    if bindings.is_empty() {
        return Vec::new();
    }

    let mut violations: Vec<(usize, String)> = Vec::new();
    let lines: Vec<&str> = contents.lines().collect();
    let mut idx = 0usize;
    while idx < lines.len() {
        // Detect a handler-ish block opener.
        let line = lines[idx];
        let opens_block = line.contains("function") || line.contains("=>") || line.contains("async");
        if !opens_block || !line.trim_end().ends_with('{') && !line.contains("{") {
            idx += 1;
            continue;
        }
        // Find the matching close brace by counting.
        let mut depth: i32 = 0;
        let mut end = idx;
        let mut started = false;
        while end < lines.len() {
            for ch in lines[end].chars() {
                if ch == '{' {
                    depth += 1;
                    started = true;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if started && depth <= 0 {
                break;
            }
            end += 1;
        }

        // Within this block, look for a mutation-binding call.
        let mut block_calls_mut = false;
        let mut binding_call_line: Option<usize> = None;
        for li in idx..=end.min(lines.len() - 1) {
            for b in &bindings {
                let pat = format!(r"\b{}\s*\(", regex::escape(b));
                let re = match Regex::new(&pat) {
                    Ok(r) => r,
                    Err(_compile_err) => continue,
                };
                if re.is_match(lines[li]) {
                    block_calls_mut = true;
                    binding_call_line = Some(li);
                    break;
                }
            }
            if block_calls_mut {
                break;
            }
        }

        if block_calls_mut {
            let call_line = match binding_call_line {
                Some(l) => l,
                None => idx,
            };
            // Look for local-mutation between block open and the first `await` after the call.
            let mut first_await: Option<usize> = None;
            for li in call_line..=end.min(lines.len() - 1) {
                if lines[li].contains("await ") {
                    first_await = Some(li);
                    break;
                }
            }
            let scan_end = match first_await {
                Some(a) => a,
                None => end.min(lines.len() - 1),
            };
            for li in idx..=scan_end {
                if LOCAL_MUTATION_RE.is_match(lines[li]) {
                    violations.push((li + 1, lines[li].to_string()));
                    break;
                }
            }
        }

        idx = end + 1;
    }
    violations
}

impl FileRule for OptimisticUpdateInCustom {
    fn name(&self) -> &'static str {
        "OptimisticUpdateInCustom"
    }

    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        _config: &FeLintState,
    ) -> Vec<Violation> {
        if !extension_is(file, "vue") && !extension_is(file, "ts") {
            return Vec::new();
        }
        if !is_user_file(file) {
            return Vec::new();
        }
        let mut out: Vec<Violation> = Vec::new();
        for (line_no, snippet) in scan_for_optimistic(contents) {
            out.push(Violation::new(
                "OptimisticUpdateInCustom",
                file.to_path_buf(),
                line_no,
                snippet_of(&snippet),
                "do not mutate local state before awaiting the server — let the action helper reconcile",
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(contents: &str) -> Vec<Violation> {
        let rule = OptimisticUpdateInCustom::new();
        let cfg = FeLintState::default();
        rule.check_file(
            &PathBuf::from("frontend/src/pages/X.vue"),
            contents,
            &cfg,
        )
    }

    #[test]
    fn flags_local_mutation_before_await() {
        let src = r#"
const updateUser = useUpdateUser()
async function handle() {
    user.value = { ...user.value, name: 'tmp' }
    updateUser({ id: 1 })
    await refetch()
}
"#;
        let v = run(src);
        assert!(!v.is_empty(), "expected violation, got none");
    }

    #[test]
    fn allows_pure_call_then_await_then_reconcile() {
        let src = r#"
const updateUser = useUpdateUser()
async function handle() {
    await updateUser({ id: 1 })
    user.value = await fetchUser()
}
"#;
        let v = run(src);
        assert!(v.is_empty(), "expected clean, got {:?}", v);
    }

    #[test]
    fn ignores_files_outside_custom() {
        let rule = OptimisticUpdateInCustom::new();
        let cfg = FeLintState::default();
        let src = r#"
const updateUser = useUpdateUser()
function h() { user.value = 1; updateUser(); }
"#;
        let v = rule.check_file(
            &PathBuf::from("frontend/src/generated/api/x.ts"),
            src,
            &cfg,
        );
        assert!(v.is_empty(), "generated/ should be ignored by this rule, got {:?}", v);
    }
}
