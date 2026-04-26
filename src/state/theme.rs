//! Theme configuration types — design tokens + PrimeVue palette preset.
//!
//! `ThemeConfig` is the state-side source of truth for two FE files that
//! are emitted by Blast's codegen pipeline (Wave B owns the codegen lane):
//!
//! - `frontend/src/styles/tokens.css` — CSS custom properties for fonts,
//!   spacing, icon sizes, container widths, responsive clamps, z-indices,
//!   transitions, and border radii.
//! - `frontend/src/plugins/primevue.ts` — PrimeVue Aura preset overlay
//!   defining the semantic primary scale and light/dark surface scales.
//!
//! Defaults here MUST round-trip to byte-identical output against the
//! current static `TOKENS_CSS` and `PRIMEVUE_TS` constants in
//! `src/codegen/frontend_scaffold.rs`. Wave B's codegen emitter and
//! parity tests treat those constants as the source-of-truth comparison
//! when the code-generated output is added.
//!
//! Typing rules (binding):
//!
//! - No `BTreeMap<String, String>` value bags. Where a small finite set
//!   of size keys is in play (`2xs`..`5xl`, `xs`..`2xl`, etc.), use the
//!   `SizeKey` enum.
//! - Numeric dimension values use `DimValue` so callers cannot smuggle
//!   raw strings ("999px-ish") through serialization.
//! - Color references inside the PrimeVue preset use `PaletteRef` for
//!   the curly-brace `{violet.500}` form; literal hex is permitted only
//!   for the surface-zero swatches because the preset file is
//!   lint-exempt for `RawColorOutsidePreset`.

use crate::error::{BlastError, BlastResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ── Atomic value types ───────────────────────────────────────────────────

/// CSS dimension value emitted into tokens.css.
///
/// Variants:
/// - `Rem(f64)`  → `"0.75rem"` (or `"1rem"` when whole)
/// - `Px(u32)`   → `"999px"` (only used for the pill radius today)
/// - `Zero`      → `"0"` (unitless, for `--app-space-0`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DimValue {
    Rem(f64),
    Px(u32),
    Zero,
}

impl Eq for DimValue {}

impl DimValue {
    /// Render the value as a CSS string. Used by Wave B's codegen lane.
    pub fn to_css(&self) -> String {
        match self {
            Self::Rem(v) => format!("{}rem", trim_rem(*v)),
            Self::Px(v) => format!("{v}px"),
            Self::Zero => "0".to_string(),
        }
    }
}

fn trim_rem(v: f64) -> String {
    // Match the existing static CSS formatting: trailing zeros stripped,
    // but a value like 1.0 emits as "1" (not "1.0"). Avoids drift like
    // `1.000rem` vs `1rem`.
    let mut s = format!("{v}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

/// `clamp(<min>, <vw>vw, <max>)` value used in responsive size tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClampValue {
    pub min: DimValue,
    pub vw: f64,
    pub max: DimValue,
}

impl Eq for ClampValue {}

impl ClampValue {
    pub fn to_css(&self) -> String {
        format!(
            "clamp({}, {}vw, {})",
            self.min.to_css(),
            trim_rem(self.vw),
            self.max.to_css()
        )
    }
}

/// 6-digit / 4-digit hex color literal. Only used inside the PrimeVue
/// preset (lint-exempt file) for surface-zero swatches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HexColor(pub String);

impl HexColor {
    /// Construct + validate. Accepts `#rgb` or `#rrggbb` (length 4 or 7,
    /// leading `#`, hex digits only).
    pub fn new<S: Into<String>>(s: S) -> BlastResult<Self> {
        let s = s.into();
        if s.len() != 4 && s.len() != 7 {
            return Err(BlastError::Invalid(format!(
                "hex color must be #rgb (len 4) or #rrggbb (len 7), got {:?}",
                s
            )));
        }
        if !s.starts_with('#') {
            return Err(BlastError::Invalid(format!(
                "hex color must start with '#', got {:?}",
                s
            )));
        }
        if !s[1..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(BlastError::Invalid(format!(
                "hex color contains non-hex char: {:?}",
                s
            )));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── Token-key enums ──────────────────────────────────────────────────────

/// Sizing scale used across font-sizes, spacing, icon sizes, container
/// widths. Not every token group uses every key — see the per-group
/// default builders for which subset is canonical.
///
/// Serialized in PascalCase per RON's bare-variant convention so the
/// state file stays human-editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SizeKey {
    /// `0` — unitless zero, only used in `spacing`.
    Zero,
    /// `3xs`
    Size3Xs,
    /// `2xs`
    Size2Xs,
    /// `xs`
    Xs,
    /// `sm`
    Sm,
    /// `md`
    Md,
    /// `lg`
    Lg,
    /// `xl`
    Xl,
    /// `2xl`
    Size2Xl,
    /// `3xl`
    Size3Xl,
    /// `4xl`
    Size4Xl,
    /// `5xl`
    Size5Xl,
    /// `6xl`
    Size6Xl,
    /// `7xl`
    Size7Xl,
}

impl SizeKey {
    /// CSS suffix used in token names (`--app-fs-<suffix>`).
    pub fn css_suffix(&self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::Size3Xs => "3xs",
            Self::Size2Xs => "2xs",
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
            Self::Size2Xl => "2xl",
            Self::Size3Xl => "3xl",
            Self::Size4Xl => "4xl",
            Self::Size5Xl => "5xl",
            Self::Size6Xl => "6xl",
            Self::Size7Xl => "7xl",
        }
    }
}

// ── Token catalog ────────────────────────────────────────────────────────

/// Font family stacks used by tokens.css.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontTokens {
    pub mono: String,
    pub sans: String,
}

impl Default for FontTokens {
    fn default() -> Self {
        Self {
            mono: "'JetBrains Mono', 'Fira Code', ui-monospace, monospace".to_string(),
            sans: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif".to_string(),
        }
    }
}

/// Full design-token catalog. All maps use `BTreeMap` for deterministic
/// iteration order in codegen output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCatalog {
    pub fonts: FontTokens,
    pub font_sizes: BTreeMap<SizeKey, DimValue>,
    pub spacing: BTreeMap<SizeKey, DimValue>,
    pub icon_sizes: BTreeMap<SizeKey, DimValue>,
    pub container_widths: BTreeMap<SizeKey, DimValue>,
    pub responsive_font_sizes: BTreeMap<String, ClampValue>,
    pub responsive_padding: BTreeMap<String, ClampValue>,
    pub z_index: BTreeMap<String, u32>,
    /// Free-form transition values like `"0.12s ease"`. The values are
    /// strings because the surface area of CSS timing functions is
    /// large and stable; constraining further would buy nothing.
    pub transitions: BTreeMap<String, String>,
    pub border_radii: BTreeMap<String, DimValue>,
}

impl Eq for TokenCatalog {}

impl Default for TokenCatalog {
    fn default() -> Self {
        Self {
            fonts: FontTokens::default(),
            font_sizes: default_font_sizes(),
            spacing: default_spacing(),
            icon_sizes: default_icon_sizes(),
            container_widths: default_container_widths(),
            responsive_font_sizes: default_responsive_font_sizes(),
            responsive_padding: default_responsive_padding(),
            z_index: default_z_index(),
            transitions: default_transitions(),
            border_radii: default_border_radii(),
        }
    }
}

fn default_font_sizes() -> BTreeMap<SizeKey, DimValue> {
    let mut m = BTreeMap::new();
    m.insert(SizeKey::Size2Xs, DimValue::Rem(0.75));
    m.insert(SizeKey::Xs, DimValue::Rem(0.8125));
    m.insert(SizeKey::Sm, DimValue::Rem(0.875));
    m.insert(SizeKey::Md, DimValue::Rem(1.0));
    m.insert(SizeKey::Lg, DimValue::Rem(1.125));
    m.insert(SizeKey::Xl, DimValue::Rem(1.25));
    m.insert(SizeKey::Size2Xl, DimValue::Rem(1.5));
    m.insert(SizeKey::Size3Xl, DimValue::Rem(1.75));
    m.insert(SizeKey::Size4Xl, DimValue::Rem(2.25));
    m.insert(SizeKey::Size5Xl, DimValue::Rem(3.5));
    m
}

fn default_spacing() -> BTreeMap<SizeKey, DimValue> {
    let mut m = BTreeMap::new();
    m.insert(SizeKey::Zero, DimValue::Zero);
    m.insert(SizeKey::Size3Xs, DimValue::Rem(0.0625));
    m.insert(SizeKey::Size2Xs, DimValue::Rem(0.125));
    m.insert(SizeKey::Xs, DimValue::Rem(0.25));
    m.insert(SizeKey::Sm, DimValue::Rem(0.375));
    m.insert(SizeKey::Md, DimValue::Rem(0.5));
    m.insert(SizeKey::Lg, DimValue::Rem(0.75));
    m.insert(SizeKey::Xl, DimValue::Rem(1.0));
    m.insert(SizeKey::Size2Xl, DimValue::Rem(1.25));
    m.insert(SizeKey::Size3Xl, DimValue::Rem(1.5));
    m.insert(SizeKey::Size4Xl, DimValue::Rem(2.0));
    m.insert(SizeKey::Size5Xl, DimValue::Rem(2.5));
    m.insert(SizeKey::Size6Xl, DimValue::Rem(3.0));
    m.insert(SizeKey::Size7Xl, DimValue::Rem(4.0));
    m
}

fn default_icon_sizes() -> BTreeMap<SizeKey, DimValue> {
    let mut m = BTreeMap::new();
    m.insert(SizeKey::Xs, DimValue::Rem(1.0));
    m.insert(SizeKey::Sm, DimValue::Rem(1.25));
    m.insert(SizeKey::Md, DimValue::Rem(1.5));
    m.insert(SizeKey::Lg, DimValue::Rem(1.75));
    m.insert(SizeKey::Xl, DimValue::Rem(2.0));
    m.insert(SizeKey::Size2Xl, DimValue::Rem(2.5));
    m
}

fn default_container_widths() -> BTreeMap<SizeKey, DimValue> {
    let mut m = BTreeMap::new();
    m.insert(SizeKey::Xs, DimValue::Rem(28.0));
    m.insert(SizeKey::Sm, DimValue::Rem(32.0));
    m.insert(SizeKey::Md, DimValue::Rem(40.0));
    m.insert(SizeKey::Lg, DimValue::Rem(50.0));
    m.insert(SizeKey::Xl, DimValue::Rem(60.0));
    m.insert(SizeKey::Size2Xl, DimValue::Rem(72.0));
    m
}

fn default_responsive_font_sizes() -> BTreeMap<String, ClampValue> {
    let mut m = BTreeMap::new();
    m.insert(
        "body-resp".into(),
        ClampValue { min: DimValue::Rem(0.9375), vw: 1.5, max: DimValue::Rem(1.125) },
    );
    m.insert(
        "sub-resp".into(),
        ClampValue { min: DimValue::Rem(1.125), vw: 1.7, max: DimValue::Rem(1.375) },
    );
    m.insert(
        "h3-resp".into(),
        ClampValue { min: DimValue::Rem(1.25), vw: 2.5, max: DimValue::Rem(1.75) },
    );
    m.insert(
        "h2-resp".into(),
        ClampValue { min: DimValue::Rem(1.5), vw: 2.6, max: DimValue::Rem(2.0) },
    );
    m.insert(
        "h1-resp".into(),
        ClampValue { min: DimValue::Rem(1.5), vw: 3.0, max: DimValue::Rem(2.25) },
    );
    m.insert(
        "display-sm".into(),
        ClampValue { min: DimValue::Rem(1.75), vw: 4.0, max: DimValue::Rem(2.75) },
    );
    m.insert(
        "display-lg".into(),
        ClampValue { min: DimValue::Rem(2.25), vw: 6.0, max: DimValue::Rem(4.5) },
    );
    m
}

fn default_responsive_padding() -> BTreeMap<String, ClampValue> {
    let mut m = BTreeMap::new();
    m.insert(
        "section-sm".into(),
        ClampValue { min: DimValue::Rem(3.0), vw: 8.0, max: DimValue::Rem(5.0) },
    );
    m.insert(
        "section-md".into(),
        ClampValue { min: DimValue::Rem(4.0), vw: 10.0, max: DimValue::Rem(7.5) },
    );
    m.insert(
        "section-lg".into(),
        ClampValue { min: DimValue::Rem(5.0), vw: 12.0, max: DimValue::Rem(10.0) },
    );
    m
}

fn default_z_index() -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();
    m.insert("content".into(), 1);
    m.insert("sidebar".into(), 20);
    m.insert("topbar".into(), 30);
    m.insert("overlay".into(), 100);
    m.insert("toast".into(), 120);
    m
}

fn default_transitions() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("fast".into(), "0.12s ease".into());
    m.insert("med".into(), "0.18s ease".into());
    m.insert("slow".into(), "0.32s ease".into());
    m
}

fn default_border_radii() -> BTreeMap<String, DimValue> {
    let mut m = BTreeMap::new();
    m.insert("sm".into(), DimValue::Rem(0.25));
    m.insert("md".into(), DimValue::Rem(0.5));
    m.insert("lg".into(), DimValue::Rem(0.75));
    m.insert("xl".into(), DimValue::Rem(1.0));
    m.insert("pill".into(), DimValue::Px(999));
    m
}

// ── PrimeVue preset ──────────────────────────────────────────────────────

/// Reference into a PrimeVue palette like `{violet.500}` or `{slate.50}`.
/// The Aura preset takes these tokens and resolves them at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteRef {
    /// PrimeVue palette name — e.g. `"violet"`, `"slate"`.
    pub palette: String,
}

impl PaletteRef {
    pub fn new<S: Into<String>>(palette: S) -> Self {
        Self { palette: palette.into() }
    }
    /// Render a single shade as the brace-token string PrimeVue expects,
    /// e.g. `{violet.500}`. Wave B's codegen emits these literally.
    pub fn brace(&self, shade: u32) -> String {
        format!("{{{}.{}}}", self.palette, shade)
    }
}

/// Direction of the surface scale. Light surfaces walk from `50` (lightest)
/// to `950` (darkest); dark surfaces walk in reverse — `50` is the
/// darkest, `950` the lightest. `Direction` lets us express that
/// relationship without duplicating shade lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceDirection {
    /// `50, 100, 200, ..., 950` — used by light surface.
    Forward,
    /// `950, 900, 800, ..., 50` — used by dark surface.
    Reversed,
}

/// Color scale reference inside the PrimeVue preset. References a palette
/// (`violet`, `slate`) and walks the standard PrimeVue shade ladder
/// (50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950) in either
/// direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorScaleRef {
    pub palette: PaletteRef,
    pub direction: SurfaceDirection,
}

impl ColorScaleRef {
    /// Standard PrimeVue shade ladder used by both Aura's primary and
    /// surface scales. Returned in render order — i.e. surface keys
    /// `50, 100, ..., 950` paired with palette shades according to
    /// `direction`.
    pub const SHADES: [u32; 11] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950];

    /// Iterator of `(surface_key, palette_shade)` pairs in render order.
    /// For `Forward`, both walk forwards (`(50, 50), (100, 100), ...`).
    /// For `Reversed`, surface key counts up while palette counts down
    /// (`(50, 950), (100, 900), ...`).
    pub fn pairs(&self) -> Vec<(u32, u32)> {
        match self.direction {
            SurfaceDirection::Forward => {
                Self::SHADES.iter().map(|s| (*s, *s)).collect()
            }
            SurfaceDirection::Reversed => {
                let len = Self::SHADES.len();
                Self::SHADES
                    .iter()
                    .enumerate()
                    .map(|(i, k)| (*k, Self::SHADES[len - 1 - i]))
                    .collect()
            }
        }
    }
}

/// PrimeVue Aura preset overlay. Defines the semantic primary scale and
/// the light/dark surface scales. Surface-zero is hex (the only literal
/// hex permitted) because PrimeVue's Aura preset uses absolute black
/// and white at the zero shade for both modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimeVuePreset {
    pub primary: ColorScaleRef,
    pub light_surface: ColorScaleRef,
    pub dark_surface: ColorScaleRef,
    pub light_surface_zero: HexColor,
    pub dark_surface_zero: HexColor,
}

impl Default for PrimeVuePreset {
    fn default() -> Self {
        // Hard-coded literals are validated above by the type's
        // constructor; the `Default` impl cannot return an error so
        // these unwraps are explicitly opted-in to. A drift in the
        // literal (e.g. mistyped) is a programmer bug, not a runtime
        // input error — and it would be caught by the round-trip test.
        Self {
            primary: ColorScaleRef {
                palette: PaletteRef::new("violet"),
                direction: SurfaceDirection::Forward,
            },
            light_surface: ColorScaleRef {
                palette: PaletteRef::new("slate"),
                direction: SurfaceDirection::Forward,
            },
            dark_surface: ColorScaleRef {
                palette: PaletteRef::new("slate"),
                direction: SurfaceDirection::Reversed,
            },
            light_surface_zero: HexColor::new("#ffffff")
                .expect("hard-coded light surface zero hex literal"), // allow: validated literal in default impl
            dark_surface_zero: HexColor::new("#0a0a0a")
                .expect("hard-coded dark surface zero hex literal"), // allow: validated literal in default impl
        }
    }
}

// ── Top-level theme config ───────────────────────────────────────────────

/// Top-level theme state. Holds the design-token catalog driving
/// `tokens.css` and the PrimeVue preset driving `plugins/primevue.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub tokens: TokenCatalog,
    pub primevue: PrimeVuePreset,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ron_roundtrip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
    {
        let config = ron::ser::PrettyConfig::new()
            .depth_limit(64)
            .indentor("  ".to_string())
            .struct_names(true);
        let s = ron::ser::to_string_pretty(value, config)
            .unwrap_or_else(|e| panic!("serialize failed: {e}\nvalue: {value:?}"));
        ron::from_str::<T>(&s)
            .unwrap_or_else(|e| panic!("deserialize failed: {e}\nRON:\n{s}"))
    }

    #[test]
    fn dim_value_to_css_emits_expected_strings() {
        assert_eq!(DimValue::Rem(0.75).to_css(), "0.75rem");
        assert_eq!(DimValue::Rem(1.0).to_css(), "1rem");
        assert_eq!(DimValue::Rem(0.8125).to_css(), "0.8125rem");
        assert_eq!(DimValue::Rem(2.5).to_css(), "2.5rem");
        assert_eq!(DimValue::Px(999).to_css(), "999px");
        assert_eq!(DimValue::Zero.to_css(), "0");
    }

    #[test]
    fn clamp_value_to_css_renders_three_args() {
        let c = ClampValue {
            min: DimValue::Rem(0.9375),
            vw: 1.5,
            max: DimValue::Rem(1.125),
        };
        assert_eq!(c.to_css(), "clamp(0.9375rem, 1.5vw, 1.125rem)");
    }

    #[test]
    fn hex_color_validates_lengths_and_chars() {
        assert!(HexColor::new("#fff").is_ok());
        assert!(HexColor::new("#ffffff").is_ok());
        assert!(HexColor::new("#0a0a0a").is_ok());
        assert!(HexColor::new("ffffff").is_err()); // missing #
        assert!(HexColor::new("#fffff").is_err()); // wrong length
        assert!(HexColor::new("#ggg").is_err()); // non-hex
    }

    #[test]
    fn color_scale_ref_pairs_forward() {
        let r = ColorScaleRef {
            palette: PaletteRef::new("violet"),
            direction: SurfaceDirection::Forward,
        };
        let pairs = r.pairs();
        assert_eq!(pairs.first(), Some(&(50, 50)));
        assert_eq!(pairs.last(), Some(&(950, 950)));
        assert_eq!(pairs.len(), 11);
    }

    #[test]
    fn color_scale_ref_pairs_reversed() {
        let r = ColorScaleRef {
            palette: PaletteRef::new("slate"),
            direction: SurfaceDirection::Reversed,
        };
        let pairs = r.pairs();
        assert_eq!(pairs.first(), Some(&(50, 950)));
        assert_eq!(pairs.last(), Some(&(950, 50)));
        // The middle entry maps surface 500 → palette 500 in either dir.
        assert!(pairs.contains(&(500, 500)));
    }

    #[test]
    fn palette_ref_brace_format() {
        let p = PaletteRef::new("violet");
        assert_eq!(p.brace(500), "{violet.500}");
        assert_eq!(p.brace(50), "{violet.50}");
    }

    #[test]
    fn default_theme_round_trips_through_ron() {
        let theme = ThemeConfig::default();
        let after = ron_roundtrip(&theme);
        assert_eq!(theme, after);
    }

    #[test]
    fn default_token_catalog_round_trips() {
        let cat = TokenCatalog::default();
        let after = ron_roundtrip(&cat);
        assert_eq!(cat, after);
    }

    #[test]
    fn default_primevue_preset_round_trips() {
        let preset = PrimeVuePreset::default();
        let after = ron_roundtrip(&preset);
        assert_eq!(preset, after);
    }

    // ── Coverage parity vs the static TOKENS_CSS constant ──────────────
    //
    // These tests prove the default ThemeConfig contains every CSS
    // custom property currently emitted by the static TOKENS_CSS string
    // in `src/codegen/frontend_scaffold.rs`. Wave B's codegen lane will
    // walk the catalog and emit byte-equivalent output; if a token key
    // is missing from the catalog, parity will fail there. Catching it
    // in this lane keeps the boundary clean.

    // Coverage parity is asserted against the keys present in
    // `src/codegen/frontend_scaffold.rs`'s `TOKENS_CSS` literal. Those
    // keys were transcribed manually here (not imported) because the
    // static constant lives in a module this lane is not allowed to
    // modify (the constant is private). Wave B's codegen lane will
    // replace the static constant with codegen and add a
    // byte-equivalent parity test against the original literal.

    #[test]
    fn font_sizes_cover_static_tokens_css_keys() {
        let cat = TokenCatalog::default();
        // Keys as they appear in static TOKENS_CSS lines `--app-fs-<key>:`.
        let static_fs_keys = [
            "2xs", "xs", "sm", "md", "lg", "xl", "2xl", "3xl", "4xl", "5xl",
        ];
        for k in static_fs_keys {
            let found = cat.font_sizes.iter().any(|(sk, _)| sk.css_suffix() == k);
            assert!(found, "catalog missing font-size key {k}");
        }
        assert_eq!(
            cat.font_sizes.len(),
            static_fs_keys.len(),
            "catalog font_sizes count drifted from static TOKENS_CSS"
        );
    }

    #[test]
    fn spacing_keys_match_static_tokens_css() {
        let cat = TokenCatalog::default();
        let static_space_keys = [
            "0", "3xs", "2xs", "xs", "sm", "md", "lg", "xl", "2xl", "3xl", "4xl",
            "5xl", "6xl", "7xl",
        ];
        for k in static_space_keys {
            let found = cat.spacing.iter().any(|(sk, _)| sk.css_suffix() == k);
            assert!(found, "catalog missing spacing key {k}");
        }
        assert_eq!(
            cat.spacing.len(),
            static_space_keys.len(),
            "catalog spacing count drifted from static TOKENS_CSS"
        );
    }

    #[test]
    fn icon_size_keys_match_static_tokens_css() {
        let cat = TokenCatalog::default();
        let static_keys = ["xs", "sm", "md", "lg", "xl", "2xl"];
        for k in static_keys {
            let found = cat.icon_sizes.iter().any(|(sk, _)| sk.css_suffix() == k);
            assert!(found, "catalog missing icon size {k}");
        }
    }

    #[test]
    fn container_keys_match_static_tokens_css() {
        let cat = TokenCatalog::default();
        let static_keys = ["xs", "sm", "md", "lg", "xl", "2xl"];
        for k in static_keys {
            let found = cat
                .container_widths
                .iter()
                .any(|(sk, _)| sk.css_suffix() == k);
            assert!(found, "catalog missing container width {k}");
        }
    }

    #[test]
    fn responsive_font_sizes_keys_match_static() {
        let cat = TokenCatalog::default();
        let expected = [
            "body-resp",
            "sub-resp",
            "h3-resp",
            "h2-resp",
            "h1-resp",
            "display-sm",
            "display-lg",
        ];
        for k in expected {
            assert!(
                cat.responsive_font_sizes.contains_key(k),
                "catalog missing responsive font key {k}"
            );
        }
    }

    #[test]
    fn responsive_padding_keys_match_static() {
        let cat = TokenCatalog::default();
        for k in ["section-sm", "section-md", "section-lg"] {
            assert!(
                cat.responsive_padding.contains_key(k),
                "catalog missing responsive padding key {k}"
            );
        }
    }

    #[test]
    fn z_index_keys_match_static() {
        let cat = TokenCatalog::default();
        for k in ["content", "sidebar", "topbar", "overlay", "toast"] {
            assert!(cat.z_index.contains_key(k), "catalog missing z-index {k}");
        }
    }

    #[test]
    fn transition_keys_match_static() {
        let cat = TokenCatalog::default();
        for k in ["fast", "med", "slow"] {
            assert!(
                cat.transitions.contains_key(k),
                "catalog missing transition {k}"
            );
        }
    }

    #[test]
    fn border_radii_keys_match_static() {
        let cat = TokenCatalog::default();
        for k in ["sm", "md", "lg", "xl", "pill"] {
            assert!(
                cat.border_radii.contains_key(k),
                "catalog missing radius {k}"
            );
        }
        // pill is a px value, the rest are rem
        match cat.border_radii.get("pill") {
            Some(DimValue::Px(999)) => {}
            other => panic!("expected pill to be Px(999), got {other:?}"),
        }
    }

    #[test]
    fn primevue_default_uses_violet_and_slate() {
        let p = PrimeVuePreset::default();
        assert_eq!(p.primary.palette.palette, "violet");
        assert_eq!(p.light_surface.palette.palette, "slate");
        assert_eq!(p.dark_surface.palette.palette, "slate");
        assert_eq!(p.primary.direction, SurfaceDirection::Forward);
        assert_eq!(p.light_surface.direction, SurfaceDirection::Forward);
        assert_eq!(p.dark_surface.direction, SurfaceDirection::Reversed);
        assert_eq!(p.light_surface_zero.as_str(), "#ffffff");
        assert_eq!(p.dark_surface_zero.as_str(), "#0a0a0a");
    }
}
