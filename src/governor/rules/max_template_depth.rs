use crate::governor::rules::helpers::{
    extension_is, extract_template_block, snippet_of,
};
use crate::governor::rules::traits::FileRule;
use crate::governor::violation::Violation;
use crate::state::FeLintState;
use std::path::Path;

pub struct MaxTemplateDepth;

impl MaxTemplateDepth {
    pub fn new() -> Self {
        Self
    }
}

/// Hand-rolled tokenizer that walks a template body counting tag opens and
/// closes. Self-closing tags (`<br/>`, `<Foo />`) do not change depth.
/// Comments (`<!-- ... -->`) are skipped. Returns the maximum simultaneous
/// nesting depth observed.
fn max_depth(body: &str) -> u32 {
    let bytes = body.as_bytes();
    let mut i = 0usize;
    let mut depth: i32 = 0;
    let mut max_d: i32 = 0;
    while i < bytes.len() {
        // Skip HTML comments.
        if i + 4 <= bytes.len() && &bytes[i..i + 4] == b"<!--" {
            let rest = &body[i + 4..];
            match rest.find("-->") {
                Some(end_rel) => i = i + 4 + end_rel + 3,
                None => return max_d as u32,
            }
            continue;
        }
        // Skip CDATA.
        if i + 9 <= bytes.len() && &bytes[i..i + 9] == b"<![CDATA[" {
            let rest = &body[i + 9..];
            match rest.find("]]>") {
                Some(end_rel) => i = i + 9 + end_rel + 3,
                None => return max_d as u32,
            }
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // We have a `<`. Determine if it's a tag, end-tag, or junk.
        let next = if i + 1 < bytes.len() {
            bytes[i + 1]
        } else {
            0
        };
        let is_end_tag = next == b'/';
        // Find the closing `>` for this tag.
        let rest = &body[i + 1..];
        let gt_rel = match rest.find('>') {
            Some(r) => r,
            None => return max_d as u32,
        };
        let inner = &rest[..gt_rel]; // does not include '<' or '>'
        // Self-closing if inner ends with '/'.
        let trimmed = inner.trim_end();
        let self_closing = trimmed.ends_with('/');

        // Check the first char after '<' is alpha — otherwise it's noise (e.g. `<` in expressions).
        let starts_with_alpha = next.is_ascii_alphabetic() || (is_end_tag
            && i + 2 < bytes.len()
            && bytes[i + 2].is_ascii_alphabetic());

        if starts_with_alpha {
            if is_end_tag {
                depth -= 1;
            } else if !self_closing {
                depth += 1;
                if depth > max_d {
                    max_d = depth;
                }
            }
        }
        // Advance past this tag.
        i = i + 1 + gt_rel + 1;
    }
    max_d.max(0) as u32
}

impl FileRule for MaxTemplateDepth {
    fn name(&self) -> &'static str {
        "MaxTemplateDepth"
    }

    fn check_file(
        &self,
        file: &Path,
        contents: &str,
        config: &FeLintState,
    ) -> Vec<Violation> {
        if !extension_is(file, "vue") {
            return Vec::new();
        }
        let block = match extract_template_block(contents) {
            Some(b) => b,
            None => return Vec::new(),
        };
        let depth = max_depth(block.inner);
        if depth <= config.max_template_depth {
            return Vec::new();
        }
        let snippet = format!("template depth {} exceeds limit", depth);
        let suggestion = format!(
            "extract a sub-component; max template depth is {}",
            config.max_template_depth
        );
        vec![Violation::new(
            "MaxTemplateDepth",
            file.to_path_buf(),
            block.start_line,
            snippet_of(&snippet),
            suggestion,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(contents: &str) -> Vec<Violation> {
        let rule = MaxTemplateDepth::new();
        let cfg = FeLintState::default(); // depth limit 5
        rule.check_file(&PathBuf::from("frontend/src/x.vue"), contents, &cfg)
    }

    #[test]
    fn allows_depth_at_limit() {
        // 5 levels deep = within limit.
        let src = r#"<template>
<a><b><c><d><e>x</e></d></c></b></a>
</template>"#;
        let v = run(src);
        assert!(v.is_empty(), "depth=5 should be ok, got {:?}", v);
    }

    #[test]
    fn flags_depth_above_limit() {
        // 6 levels deep = over limit.
        let src = r#"<template>
<a><b><c><d><e><f>x</f></e></d></c></b></a>
</template>"#;
        let v = run(src);
        assert_eq!(v.len(), 1, "depth=6 should be flagged");
    }

    #[test]
    fn ignores_self_closing_tags_as_depth() {
        let src = r#"<template>
<a><b><c><d><br/><img/></d></c></b></a>
</template>"#;
        let v = run(src);
        // depth=4 well under limit
        assert!(v.is_empty(), "got {:?}", v);
    }

    #[test]
    fn skips_comments() {
        let src = r#"<template>
<a><b><!-- <c><d><e><f><g><h> --></b></a>
</template>"#;
        let v = run(src);
        assert!(v.is_empty(), "comments must not contribute depth, got {:?}", v);
    }

    #[test]
    fn handles_single_root() {
        let src = r#"<template><div>hi</div></template>"#;
        let v = run(src);
        assert!(v.is_empty());
    }

    #[test]
    fn handles_no_template_block() {
        let src = r#"<script>const x = 1</script>"#;
        let v = run(src);
        assert!(v.is_empty());
    }
}
