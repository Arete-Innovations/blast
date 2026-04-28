use std::path::Path;

use crate::{
    governor::{
        rules::{
            helpers::{extension_is, extract_template_block, snippet_of},
            traits::FileRule,
        },
        violation::Violation,
    },
    state::FeLintState,
};

pub struct MaxTemplateLoc;

impl MaxTemplateLoc {
    pub fn new() -> Self {
        Self
    }
}

impl FileRule for MaxTemplateLoc {
    fn name(&self) -> &'static str {
        "MaxTemplateLoc"
    }

    fn check_file(&self, file: &Path, contents: &str, config: &FeLintState) -> Vec<Violation> {
        if !extension_is(file, "vue") {
            return Vec::new();
        }
        let block = match extract_template_block(contents) {
            Some(b) => b,
            None => return Vec::new(),
        };
        let inner_lines = block.inner.matches('\n').count() as u32 + 1;
        if inner_lines <= config.max_template_loc {
            return Vec::new();
        }
        let snippet = format!("template has {} lines", inner_lines);
        let suggestion = format!("split into sub-components; max template LOC is {}", config.max_template_loc);
        vec![Violation::new("MaxTemplateLoc", file.to_path_buf(), block.start_line, snippet_of(&snippet), suggestion)]
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn run_with_limit(contents: &str, limit: u32) -> Vec<Violation> {
        let rule = MaxTemplateLoc::new();
        let mut cfg = FeLintState::default();
        cfg.max_template_loc = limit;
        rule.check_file(&PathBuf::from("frontend/src/x.vue"), contents, &cfg)
    }

    #[test]
    fn allows_short_template() {
        let src = "<template>\n<div>x</div>\n</template>";
        let v = run_with_limit(src, 200);
        assert!(v.is_empty());
    }

    #[test]
    fn flags_long_template() {
        let body: String = (0..50).map(|_| "  <p>line</p>\n").collect();
        let src = format!("<template>\n{body}</template>");
        let v = run_with_limit(&src, 10);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn ignores_when_no_template_block() {
        let src = "<script>const x = 1</script>";
        let v = run_with_limit(src, 5);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_non_vue_files() {
        let rule = MaxTemplateLoc::new();
        let cfg = FeLintState::default();
        let v = rule.check_file(&PathBuf::from("frontend/src/x.ts"), "<template>\n<a/>\n</template>", &cfg);
        assert!(v.is_empty());
    }
}
