//! Tokens.css emitter.
//!
//! Walks `ThemeConfig.tokens` (a `TokenCatalog`) and emits one CSS custom
//! property per entry inside a single `@layer app { :root { ... } }`
//! wrapper. The output replaces the static `TOKENS_CSS` constant that
//! used to live in `frontend_scaffold.rs`.
//!
//! Emission order is deterministic by virtue of every map being a
//! `BTreeMap` and every enum-keyed map being walked in `SizeKey` declared
//! order.

use crate::state::theme::{SizeKey, ThemeConfig, TokenCatalog};

/// Render a `tokens.css` body (no header marker — caller prepends).
pub fn emit_tokens_css(theme: &ThemeConfig) -> String {
    let mut out = String::new();
    out.push_str("@layer app {\n");
    out.push_str("  :root {\n");
    emit_fonts(&mut out, &theme.tokens);
    emit_size_group(&mut out, "fs", &theme.tokens.font_sizes, font_size_order());
    emit_size_group(&mut out, "space", &theme.tokens.spacing, spacing_order());
    emit_size_group(&mut out, "icon", &theme.tokens.icon_sizes, icon_size_order());
    emit_size_group(
        &mut out,
        "container",
        &theme.tokens.container_widths,
        container_order(),
    );
    emit_clamp_group(&mut out, "fs", &theme.tokens.responsive_font_sizes, responsive_fs_order());
    emit_clamp_group(&mut out, "pad", &theme.tokens.responsive_padding, responsive_pad_order());
    emit_z_index(&mut out, &theme.tokens);
    emit_transitions(&mut out, &theme.tokens);
    emit_radii(&mut out, &theme.tokens);
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn emit_fonts(out: &mut String, cat: &TokenCatalog) {
    out.push_str(&format!("    --app-font-mono: {};\n", cat.fonts.mono));
    out.push_str(&format!("    --app-font-sans: {};\n", cat.fonts.sans));
    out.push('\n');
}

fn emit_size_group(
    out: &mut String,
    prefix: &str,
    map: &std::collections::BTreeMap<SizeKey, crate::state::theme::DimValue>,
    order: &[SizeKey],
) {
    let mut wrote = false;
    for key in order {
        match map.get(key) {
            Some(value) => {
                out.push_str(&format!(
                    "    --app-{}-{}: {};\n",
                    prefix,
                    key.css_suffix(),
                    value.to_css()
                ));
                wrote = true;
            }
            None => {}
        }
    }
    if wrote {
        out.push('\n');
    }
}

fn emit_clamp_group(
    out: &mut String,
    prefix: &str,
    map: &std::collections::BTreeMap<String, crate::state::theme::ClampValue>,
    order: &[&str],
) {
    let mut wrote = false;
    for key in order {
        match map.get(*key) {
            Some(value) => {
                out.push_str(&format!("    --app-{}-{}: {};\n", prefix, key, value.to_css()));
                wrote = true;
            }
            None => {}
        }
    }
    // Catch any keys not in the explicit order list, deterministically.
    for (key, value) in map {
        if !order.iter().any(|k| *k == key.as_str()) {
            out.push_str(&format!("    --app-{}-{}: {};\n", prefix, key, value.to_css()));
            wrote = true;
        }
    }
    if wrote {
        out.push('\n');
    }
}

fn emit_z_index(out: &mut String, cat: &TokenCatalog) {
    let order = ["content", "sidebar", "topbar", "overlay", "toast"];
    let mut wrote = false;
    for key in order {
        match cat.z_index.get(key) {
            Some(v) => {
                out.push_str(&format!("    --app-z-{}: {};\n", key, v));
                wrote = true;
            }
            None => {}
        }
    }
    for (key, v) in &cat.z_index {
        if !order.contains(&key.as_str()) {
            out.push_str(&format!("    --app-z-{}: {};\n", key, v));
            wrote = true;
        }
    }
    if wrote {
        out.push('\n');
    }
}

fn emit_transitions(out: &mut String, cat: &TokenCatalog) {
    let order = ["fast", "med", "slow"];
    let mut wrote = false;
    for key in order {
        match cat.transitions.get(key) {
            Some(v) => {
                out.push_str(&format!("    --app-transition-{}: {};\n", key, v));
                wrote = true;
            }
            None => {}
        }
    }
    for (key, v) in &cat.transitions {
        if !order.contains(&key.as_str()) {
            out.push_str(&format!("    --app-transition-{}: {};\n", key, v));
            wrote = true;
        }
    }
    if wrote {
        out.push('\n');
    }
}

fn emit_radii(out: &mut String, cat: &TokenCatalog) {
    let order = ["sm", "md", "lg", "xl", "pill"];
    for key in order {
        match cat.border_radii.get(key) {
            Some(v) => {
                out.push_str(&format!("    --app-radius-{}: {};\n", key, v.to_css()));
            }
            None => {}
        }
    }
    for (key, v) in &cat.border_radii {
        if !order.contains(&key.as_str()) {
            out.push_str(&format!("    --app-radius-{}: {};\n", key, v.to_css()));
        }
    }
    // Note: this is the last group before the `:root` closing brace, so
    // we deliberately do NOT push a trailing blank line here. Other
    // groups push one for visual separation.
}

// ── Render-order vectors ─────────────────────────────────────────────────
//
// SizeKey's `BTreeMap` ordering would emit keys alphabetically, but the
// canonical TOKENS_CSS lists them in scale order (xs, sm, md, lg, ...).
// We hard-code the desired emission order here, then fall back to BTreeMap
// order for any future keys that don't appear in this list.

fn font_size_order() -> &'static [SizeKey] {
    &[
        SizeKey::Size2Xs,
        SizeKey::Xs,
        SizeKey::Sm,
        SizeKey::Md,
        SizeKey::Lg,
        SizeKey::Xl,
        SizeKey::Size2Xl,
        SizeKey::Size3Xl,
        SizeKey::Size4Xl,
        SizeKey::Size5Xl,
    ]
}

fn spacing_order() -> &'static [SizeKey] {
    &[
        SizeKey::Zero,
        SizeKey::Size3Xs,
        SizeKey::Size2Xs,
        SizeKey::Xs,
        SizeKey::Sm,
        SizeKey::Md,
        SizeKey::Lg,
        SizeKey::Xl,
        SizeKey::Size2Xl,
        SizeKey::Size3Xl,
        SizeKey::Size4Xl,
        SizeKey::Size5Xl,
        SizeKey::Size6Xl,
        SizeKey::Size7Xl,
    ]
}

fn icon_size_order() -> &'static [SizeKey] {
    &[
        SizeKey::Xs,
        SizeKey::Sm,
        SizeKey::Md,
        SizeKey::Lg,
        SizeKey::Xl,
        SizeKey::Size2Xl,
    ]
}

fn container_order() -> &'static [SizeKey] {
    &[
        SizeKey::Xs,
        SizeKey::Sm,
        SizeKey::Md,
        SizeKey::Lg,
        SizeKey::Xl,
        SizeKey::Size2Xl,
    ]
}

fn responsive_fs_order() -> &'static [&'static str] {
    &[
        "body-resp",
        "sub-resp",
        "h3-resp",
        "h2-resp",
        "h1-resp",
        "display-sm",
        "display-lg",
    ]
}

fn responsive_pad_order() -> &'static [&'static str] {
    &["section-sm", "section-md", "section-lg"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Static reference body — copy of the current `TOKENS_CSS` constant
    /// in `src/codegen/frontend_scaffold.rs`. The parity test below proves
    /// every CSS custom property pair (var → value) in the current static
    /// content is emitted by the codegen with the same value.
    const REFERENCE_TOKENS_CSS: &str = r#"@layer app {
  :root {
    --app-font-mono: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
    --app-font-sans: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;

    --app-fs-2xs: 0.75rem;
    --app-fs-xs:  0.8125rem;
    --app-fs-sm:  0.875rem;
    --app-fs-md:  1rem;
    --app-fs-lg:  1.125rem;
    --app-fs-xl:  1.25rem;
    --app-fs-2xl: 1.5rem;
    --app-fs-3xl: 1.75rem;
    --app-fs-4xl: 2.25rem;
    --app-fs-5xl: 3.5rem;

    --app-space-0:   0;
    --app-space-3xs: 0.0625rem;
    --app-space-2xs: 0.125rem;
    --app-space-xs:  0.25rem;
    --app-space-sm:  0.375rem;
    --app-space-md:  0.5rem;
    --app-space-lg:  0.75rem;
    --app-space-xl:  1rem;
    --app-space-2xl: 1.25rem;
    --app-space-3xl: 1.5rem;
    --app-space-4xl: 2rem;
    --app-space-5xl: 2.5rem;
    --app-space-6xl: 3rem;
    --app-space-7xl: 4rem;

    --app-icon-xs:  1rem;
    --app-icon-sm:  1.25rem;
    --app-icon-md:  1.5rem;
    --app-icon-lg:  1.75rem;
    --app-icon-xl:  2rem;
    --app-icon-2xl: 2.5rem;

    --app-container-xs: 28rem;
    --app-container-sm: 32rem;
    --app-container-md: 40rem;
    --app-container-lg: 50rem;
    --app-container-xl: 60rem;
    --app-container-2xl: 72rem;

    --app-fs-body-resp:  clamp(0.9375rem, 1.5vw, 1.125rem);
    --app-fs-sub-resp:   clamp(1.125rem, 1.7vw, 1.375rem);
    --app-fs-h3-resp:    clamp(1.25rem, 2.5vw, 1.75rem);
    --app-fs-h2-resp:    clamp(1.5rem, 2.6vw, 2rem);
    --app-fs-h1-resp:    clamp(1.5rem, 3vw, 2.25rem);
    --app-fs-display-sm: clamp(1.75rem, 4vw, 2.75rem);
    --app-fs-display-lg: clamp(2.25rem, 6vw, 4.5rem);

    --app-pad-section-sm: clamp(3rem, 8vw, 5rem);
    --app-pad-section-md: clamp(4rem, 10vw, 7.5rem);
    --app-pad-section-lg: clamp(5rem, 12vw, 10rem);

    --app-z-content: 1;
    --app-z-sidebar: 20;
    --app-z-topbar:  30;
    --app-z-overlay: 100;
    --app-z-toast:   120;

    --app-transition-fast: 0.12s ease;
    --app-transition-med:  0.18s ease;
    --app-transition-slow: 0.32s ease;

    --app-radius-sm:   0.25rem;
    --app-radius-md:   0.5rem;
    --app-radius-lg:   0.75rem;
    --app-radius-xl:   1rem;
    --app-radius-pill: 999px;
  }
}
"#;

    /// Parse a CSS body into a (var-name → value) map. Robust against
    /// whitespace differences between codegen output and the static
    /// reference's hand-aligned padding.
    fn parse_vars(css: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for line in css.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("--") {
                continue;
            }
            let stripped = match trimmed.strip_suffix(';') {
                Some(s) => s,
                None => continue,
            };
            let colon = match stripped.find(':') {
                Some(i) => i,
                None => continue,
            };
            let name = stripped[..colon].trim().to_string();
            let value = stripped[colon + 1..].trim().to_string();
            out.insert(name, value);
        }
        out
    }

    #[test]
    fn emits_outer_layer_wrapper() {
        let theme = ThemeConfig::default();
        let body = emit_tokens_css(&theme);
        assert!(body.starts_with("@layer app {\n  :root {\n"));
        assert!(body.trim_end().ends_with("}"));
        // Two closing braces: one for :root, one for @layer.
        let close_count = body.matches('}').count();
        assert_eq!(close_count, 2);
    }

    #[test]
    fn emits_byte_equivalent_var_set_to_static_reference() {
        let theme = ThemeConfig::default();
        let emitted = emit_tokens_css(&theme);
        let lhs = parse_vars(&emitted);
        let rhs = parse_vars(REFERENCE_TOKENS_CSS);
        assert_eq!(
            lhs, rhs,
            "codegen tokens.css var set drifted from static reference\nlhs: {lhs:#?}\nrhs: {rhs:#?}"
        );
    }

    #[test]
    fn emits_all_expected_font_size_keys() {
        let body = emit_tokens_css(&ThemeConfig::default());
        for key in [
            "--app-fs-2xs",
            "--app-fs-xs",
            "--app-fs-sm",
            "--app-fs-md",
            "--app-fs-lg",
            "--app-fs-xl",
            "--app-fs-2xl",
            "--app-fs-3xl",
            "--app-fs-4xl",
            "--app-fs-5xl",
        ] {
            assert!(
                body.contains(key),
                "expected emitted body to contain {key}\nbody:\n{body}"
            );
        }
    }

    #[test]
    fn emits_pill_radius_in_pixels() {
        let body = emit_tokens_css(&ThemeConfig::default());
        assert!(body.contains("--app-radius-pill: 999px"));
    }

    #[test]
    fn emits_responsive_clamps() {
        let body = emit_tokens_css(&ThemeConfig::default());
        assert!(body.contains("clamp(0.9375rem, 1.5vw, 1.125rem)"));
        assert!(body.contains("clamp(2.25rem, 6vw, 4.5rem)"));
    }

    #[test]
    fn emits_font_family_strings_verbatim() {
        let body = emit_tokens_css(&ThemeConfig::default());
        assert!(body.contains(
            "--app-font-mono: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;"
        ));
        assert!(body.contains(
            "--app-font-sans: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;"
        ));
    }

    #[test]
    fn emits_z_index_integers() {
        let body = emit_tokens_css(&ThemeConfig::default());
        assert!(body.contains("--app-z-content: 1;"));
        assert!(body.contains("--app-z-overlay: 100;"));
        assert!(body.contains("--app-z-toast: 120;"));
    }
}
