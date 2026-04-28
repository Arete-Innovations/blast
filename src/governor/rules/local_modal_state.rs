use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    governor::{
        rules::{
            helpers::{extension_is, snippet_of},
            traits::FileRule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

lazy_static! {
    /// Captures the v-model:visible identifier on a modal-overlay element.
    /// Matches Dialog, Drawer, Sidebar, Modal — the PrimeVue modal primitives
    /// catalogued in SPEC_GOVERNOR.md.
    static ref MODAL_VISIBLE_RE: Regex = match Regex::new(
        r#"<(Dialog|Drawer|Sidebar|Modal)\b[^>]*\bv-model:visible\s*=\s*"([^"]+)""#
    ) {
        Ok(r) => r,
        Err(_re_err) => panic!("LocalModalState modal regex failed to compile"), // allow: const pattern, infallible
    };
}

pub struct LocalModalState;

impl LocalModalState {
    pub fn new() -> Self {
        Self
    }
}

fn is_local_ref_decl(script: &str, ident: &str) -> bool {
    // Match `const showEdit = ref(...)`, `let showEdit = reactive(...)`,
    // ignoring useQuery* composables.
    let pat = format!(r"\b(?:const|let|var)\s+{}\s*=\s*(ref|reactive|shallowRef)\s*\(", regex::escape(ident));
    let re = match Regex::new(&pat) {
        Ok(r) => r,
        Err(_compile_err) => return false,
    };
    re.is_match(script)
}

fn extract_script_block(contents: &str) -> &str {
    let lower = contents.to_ascii_lowercase();
    let open = match lower.find("<script") {
        Some(i) => i,
        None => return "",
    };
    let after = open + "<script".len();
    let rel = match lower[after..].find('>') {
        Some(i) => i,
        None => return "",
    };
    let body_start = after + rel + 1;
    let close = match lower[body_start..].find("</script") {
        Some(i) => i,
        None => return &contents[body_start..],
    };
    &contents[body_start..body_start + close]
}

fn line_of_offset(contents: &str, offset: usize) -> usize {
    contents[..offset.min(contents.len())].lines().count().max(1)
}

impl FileRule for LocalModalState {
    fn name(&self) -> &'static str {
        "LocalModalState"
    }

    fn check_file(&self, file: &Path, contents: &str, _config: &FeLintState) -> Vec<Violation> {
        if !extension_is(file, "vue") {
            return Vec::new();
        }
        let script = extract_script_block(contents);
        let mut out: Vec<Violation> = Vec::new();
        for caps in MODAL_VISIBLE_RE.captures_iter(contents) {
            let ident = match caps.get(2) {
                Some(m) => m.as_str(),
                None => continue,
            };
            // Skip property accesses like dialog.visible.
            if ident.contains('.') {
                continue;
            }
            if !is_local_ref_decl(script, ident) {
                continue;
            }
            let snippet = match caps.get(0) {
                Some(m) => m.as_str(),
                None => continue,
            };
            let offset = match caps.get(0) {
                Some(m) => m.start(),
                None => continue, // captures_iter always yields a match, defensive only
            };
            out.push(Violation::new(
                "LocalModalState",
                file.to_path_buf(),
                line_of_offset(contents, offset),
                snippet_of(snippet),
                "modal state belongs in the URL — use useQueryDialog/useQueryDrawer composables",
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run(contents: &str) -> Vec<Violation> {
        let rule = LocalModalState::new();
        let cfg = FeLintState::default();
        rule.check_file(&PathBuf::from("frontend/src/pages/X.vue"), contents, &cfg)
    }

    #[test]
    fn flags_dialog_with_local_ref_visible() {
        let src = r#"
<script setup>
const showEdit = ref(false)
</script>
<template>
  <Dialog v-model:visible="showEdit" />
</template>
"#;
        let v = run(src);
        assert_eq!(v.len(), 1, "expected 1 violation, got {:?}", v);
    }

    #[test]
    fn allows_dialog_with_composable_source() {
        let src = r#"
<script setup>
const dialog = useQueryDialog('user-edit')
</script>
<template>
  <Dialog v-model:visible="dialog.visible" />
</template>
"#;
        let v = run(src);
        assert!(v.is_empty(), "expected clean, got {:?}", v);
    }

    #[test]
    fn allows_dialog_when_ident_not_local_ref() {
        let src = r#"
<script setup>
const props = defineProps<{ open: boolean }>()
</script>
<template>
  <Dialog v-model:visible="props.open" />
</template>
"#;
        let v = run(src);
        assert!(v.is_empty(), "props-driven visible should not be flagged, got {:?}", v);
    }

    #[test]
    fn flags_drawer_with_local_reactive_visible() {
        let src = r#"
<script setup>
const open = reactive({ value: false })
</script>
<template>
  <Drawer v-model:visible="open" />
</template>
"#;
        let v = run(src);
        assert_eq!(v.len(), 1, "expected drawer violation, got {:?}", v);
    }
}
