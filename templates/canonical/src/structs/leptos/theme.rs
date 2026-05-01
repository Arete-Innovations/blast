use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    System,
}

impl Theme {
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::System => "system",
        }
    }

    pub fn from_cookie(s: &str) -> Self {
        match Self::try_from_cookie(s) {
            Some(theme) => theme,
            None => Theme::System,
        }
    }

    fn try_from_cookie(s: &str) -> Option<Self> {
        match s.trim() {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            "system" => Some(Theme::System),
            other => {
                tracing::debug!(theme_cookie_value = other, "unknown theme cookie variant; falling back to System");
                None
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::System
    }
}
