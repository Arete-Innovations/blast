//! PrimeVue preset emitter.
//!
//! Walks `ThemeConfig.primevue` (a `PrimeVuePreset`) and emits the TS
//! file that registers a PrimeVue Aura preset overlay. Output replaces
//! the static `PRIMEVUE_TS` constant that used to live in
//! `frontend_scaffold.rs`.
//!
//! Surface-zero shades are the only literal hex values permitted in the
//! generated output — PrimeVue's Aura preset always uses an absolute
//! white (`#ffffff`) and an absolute black (`#0a0a0a`) at the zero key
//! for light/dark surfaces respectively. The Governor lint rule
//! `RawColorOutsidePreset` exempts this preset file specifically.

use crate::state::theme::{ColorScaleRef, PrimeVuePreset, ThemeConfig};

/// Render a `primevue.ts` body (no header marker — caller prepends).
pub fn emit_primevue_ts(theme: &ThemeConfig) -> String {
    let preset = &theme.primevue;
    let mut out = String::new();
    out.push_str("import type { App } from 'vue'\n");
    out.push_str("import PrimeVueConfig from 'primevue/config'\n");
    out.push_str("import Aura from '@primeuix/themes/aura'\n");
    out.push_str("import { definePreset } from '@primeuix/themes'\n");
    out.push('\n');
    out.push_str("const PRESET_SEMANTIC = definePreset(Aura, {\n");
    out.push_str("  semantic: {\n");
    emit_primary(&mut out, &preset.primary);
    out.push_str(",\n");
    emit_color_scheme(&mut out, preset);
    out.push_str("  }\n");
    out.push_str("})\n");
    out.push('\n');
    out.push_str("export default function installPrimeVue(app: App): void {\n");
    out.push_str("  app.use(PrimeVueConfig, {\n");
    out.push_str("    theme: {\n");
    out.push_str("      preset: PRESET_SEMANTIC,\n");
    out.push_str("      options: {\n");
    out.push_str("        cssLayer: { name: 'primevue', order: 'reset, primevue, app' }\n");
    out.push_str("      }\n");
    out.push_str("    }\n");
    out.push_str("  })\n");
    out.push_str("}\n");
    out
}

fn emit_primary(out: &mut String, primary: &ColorScaleRef) {
    out.push_str("    primary: {\n");
    let pairs = primary.pairs();
    let last_idx = pairs.len() - 1;
    for (i, (surface_key, palette_shade)) in pairs.iter().enumerate() {
        let comma = if i == last_idx { "" } else { "," };
        out.push_str(&format!(
            "      {}'{}'{}\n",
            aligned_shade_key(*surface_key),
            primary.palette.brace(*palette_shade),
            comma,
        ));
    }
    out.push_str("    }");
}

fn emit_color_scheme(out: &mut String, preset: &PrimeVuePreset) {
    out.push_str("    colorScheme: {\n");
    emit_surface_block(
        out,
        "light",
        &preset.light_surface,
        preset.light_surface_zero.as_str(),
        true,
    );
    emit_surface_block(
        out,
        "dark",
        &preset.dark_surface,
        preset.dark_surface_zero.as_str(),
        false,
    );
    out.push_str("    }\n");
}

fn emit_surface_block(
    out: &mut String,
    mode: &str,
    surface: &ColorScaleRef,
    surface_zero_hex: &str,
    trailing_comma: bool,
) {
    out.push_str(&format!("      {}: {{\n", mode));
    out.push_str("        surface: {\n");
    out.push_str(&format!("          0:   '{}',\n", surface_zero_hex));
    let pairs = surface.pairs();
    let last_idx = pairs.len() - 1;
    for (i, (surface_key, palette_shade)) in pairs.iter().enumerate() {
        let comma = if i == last_idx { "" } else { "," };
        out.push_str(&format!(
            "          {}'{}'{}\n",
            aligned_shade_key(*surface_key),
            surface.palette.brace(*palette_shade),
            comma,
        ));
    }
    out.push_str("        }\n");
    let close = if trailing_comma { "      },\n" } else { "      }\n" };
    out.push_str(close);
}

/// Shade key plus colon plus alignment padding, as a single string slot
/// the caller can drop in front of the value. Canonical preset shapes:
///
/// - 2-digit shades: `"50:  "` (`50` + `:` + two spaces) → 5 chars
/// - 3-digit shades: `"100: "` (`100` + `:` + one space) → 5 chars
///
/// Width 5 makes the value column line up at column 11 (after the
/// 6-space indent for `primary` entries) or column 15 (after the
/// 10-space indent for surface entries). Caller supplies the indent.
fn aligned_shade_key(shade: u32) -> String {
    let mut s = format!("{shade}:");
    while s.len() < 5 {
        s.push(' ');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Static reference body — copy of the current `PRIMEVUE_TS` constant
    /// in `src/codegen/frontend_scaffold.rs`. Used to assert the codegen
    /// emitter produces semantically equivalent output (same shade
    /// references, same surface-zero hex literals, same import lines).
    const REFERENCE_PRIMEVUE_TS: &str = r#"import type { App } from 'vue'
import PrimeVueConfig from 'primevue/config'
import Aura from '@primeuix/themes/aura'
import { definePreset } from '@primeuix/themes'

const PRESET_SEMANTIC = definePreset(Aura, {
  semantic: {
    primary: {
      50:  '{violet.50}',
      100: '{violet.100}',
      200: '{violet.200}',
      300: '{violet.300}',
      400: '{violet.400}',
      500: '{violet.500}',
      600: '{violet.600}',
      700: '{violet.700}',
      800: '{violet.800}',
      900: '{violet.900}',
      950: '{violet.950}'
    },
    colorScheme: {
      light: {
        surface: {
          0:   '#ffffff',
          50:  '{slate.50}',
          100: '{slate.100}',
          200: '{slate.200}',
          300: '{slate.300}',
          400: '{slate.400}',
          500: '{slate.500}',
          600: '{slate.600}',
          700: '{slate.700}',
          800: '{slate.800}',
          900: '{slate.900}',
          950: '{slate.950}'
        }
      },
      dark: {
        surface: {
          0:   '#0a0a0a',
          50:  '{slate.950}',
          100: '{slate.900}',
          200: '{slate.800}',
          300: '{slate.700}',
          400: '{slate.600}',
          500: '{slate.500}',
          600: '{slate.400}',
          700: '{slate.300}',
          800: '{slate.200}',
          900: '{slate.100}',
          950: '{slate.50}'
        }
      }
    }
  }
})

export default function installPrimeVue(app: App): void {
  app.use(PrimeVueConfig, {
    theme: {
      preset: PRESET_SEMANTIC,
      options: {
        cssLayer: { name: 'primevue', order: 'reset, primevue, app' }
      }
    }
  })
}
"#;

    #[test]
    fn contains_required_landmarks() {
        let body = emit_primevue_ts(&ThemeConfig::default());
        // Imports.
        assert!(body.contains("import type { App } from 'vue'"));
        assert!(body.contains("import PrimeVueConfig from 'primevue/config'"));
        assert!(body.contains("import Aura from '@primeuix/themes/aura'"));
        assert!(body.contains("import { definePreset } from '@primeuix/themes'"));
        // Preset shape.
        assert!(body.contains("definePreset(Aura,"));
        assert!(body.contains("primary:"));
        assert!(body.contains("colorScheme:"));
        assert!(body.contains("light:"));
        assert!(body.contains("dark:"));
        // Surface zero hex literals.
        assert!(body.contains("'#ffffff'"));
        assert!(body.contains("'#0a0a0a'"));
        // PrimeVue brace tokens (sanity check: a couple of shades).
        assert!(body.contains("{violet.500}"));
        assert!(body.contains("{slate.500}"));
        assert!(body.contains("{slate.50}"));
        assert!(body.contains("{slate.950}"));
    }

    #[test]
    fn primary_walks_violet_forward() {
        let body = emit_primevue_ts(&ThemeConfig::default());
        // Forward primary scale: surface key matches palette shade.
        for s in [50u32, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] {
            let needle = format!("'{{violet.{}}}'", s);
            assert!(
                body.contains(&needle),
                "expected primary to reference {needle}\nbody:\n{body}"
            );
        }
    }

    #[test]
    fn light_surface_walks_slate_forward() {
        let body = emit_primevue_ts(&ThemeConfig::default());
        // light.surface.50 is {slate.50}, light.surface.950 is {slate.950}.
        // Sample the endpoints + middle.
        assert!(body.contains("50:  '{slate.50}'"));
        assert!(body.contains("950: '{slate.950}'"));
        assert!(body.contains("500: '{slate.500}'"));
    }

    #[test]
    fn dark_surface_walks_slate_reversed() {
        let body = emit_primevue_ts(&ThemeConfig::default());
        // dark.surface.50 → {slate.950}, dark.surface.950 → {slate.50}.
        // We need to find these occurrences inside the dark block; a
        // substring check is enough since the strings are unique.
        assert!(body.contains("50:  '{slate.950}'"));
        assert!(body.contains("950: '{slate.50}'"));
    }

    #[test]
    fn includes_install_function() {
        let body = emit_primevue_ts(&ThemeConfig::default());
        assert!(body.contains("export default function installPrimeVue"));
        assert!(body.contains("preset: PRESET_SEMANTIC"));
        assert!(body.contains("cssLayer: { name: 'primevue', order: 'reset, primevue, app' }"));
    }

    #[test]
    fn output_matches_static_reference_byte_for_byte() {
        // The codegen targets byte-equivalence to the existing static
        // PRIMEVUE_TS so the migration introduces no diff for the user.
        let body = emit_primevue_ts(&ThemeConfig::default());
        assert_eq!(
            body, REFERENCE_PRIMEVUE_TS,
            "codegen primevue.ts drifted from static reference\nemitted:\n{body}"
        );
    }
}
