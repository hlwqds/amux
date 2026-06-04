use std::fs;
use std::path::PathBuf;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::data_dir;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeName {
    #[default]
    Dark,
    Light,
    Mocha,
    Custom(String),
}
impl ThemeName {
    pub fn cycle(&self) -> Self {
        let mut themes = vec![ThemeName::Dark, ThemeName::Light, ThemeName::Mocha];
        // Append discovered custom themes
        if let Some(customs) = discover_custom_themes() {
            themes.extend(customs);
        }
        let idx = themes.iter().position(|t| t == self).unwrap_or(0);
        themes[(idx + 1) % themes.len()].clone()
    }

    pub fn label(&self) -> &str {
        match self {
            ThemeName::Dark => "Dark",
            ThemeName::Light => "Light",
            ThemeName::Mocha => "Catppuccin Mocha",
            ThemeName::Custom(name) => name,
        }
    }

    pub fn theme(&self) -> Theme {
        match self {
            ThemeName::Dark => Theme::dark(),
            ThemeName::Light => Theme::light(),
            ThemeName::Mocha => Theme::mocha(),
            ThemeName::Custom(name) => load_custom_theme(name).unwrap_or_else(Theme::dark),
        }
    }
}

/// Serializable theme file format — all fields optional, missing fields fall back to Dark theme.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ThemeFile {
    pub sidebar_bg: Option<String>,
    pub sidebar_title: Option<String>,
    pub sidebar_text: Option<String>,
    pub sidebar_dim: Option<String>,
    pub sidebar_highlight: Option<String>,
    pub sidebar_selected: Option<String>,

    pub chat_border: Option<String>,
    pub chat_title: Option<String>,

    pub agent_claude: Option<String>,
    pub agent_codex: Option<String>,
    pub agent_omp: Option<String>,
    pub status_running: Option<String>,
    pub status_done: Option<String>,
    pub status_error: Option<String>,

    pub popup_border: Option<String>,
    pub popup_title: Option<String>,
    pub popup_text: Option<String>,
    pub popup_hint: Option<String>,

    pub accent: Option<String>,
    pub dim: Option<String>,
    pub bold_text: Option<String>,
    pub input_cursor: Option<String>,
}

/// Parse a color string into a ratatui Color.
/// Supports: named colors ("Cyan", "Red", ...), hex ("#rrggbb"), and "Rgb(r,g,b)".
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();

    // Rgb(r,g,b) format
    if let Some(inner) = s.strip_prefix("Rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            return Some(Color::Rgb(r, g, b));
        }
        return None;
    }

    // Hex #RRGGBB format
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
        return None;
    }

    // Named colors — match ratatui::style::Color enum variants
    Some(match s {
        "Reset" => Color::Reset,
        "Black" => Color::Black,
        "Red" => Color::Red,
        "Green" => Color::Green,
        "Yellow" => Color::Yellow,
        "Blue" => Color::Blue,
        "Magenta" => Color::Magenta,
        "Cyan" => Color::Cyan,
        "Gray" => Color::Gray,
        "DarkGray" => Color::DarkGray,
        "LightRed" => Color::LightRed,
        "LightGreen" => Color::LightGreen,
        "LightYellow" => Color::LightYellow,
        "LightBlue" => Color::LightBlue,
        "LightMagenta" => Color::LightMagenta,
        "LightCyan" => Color::LightCyan,
        "White" => Color::White,
        _ => return None,
    })
}

/// Directory where custom theme JSON files live.
fn themes_dir() -> PathBuf {
    data_dir().join("themes")
}

/// Load a custom theme by name from `~/.local/share/amux/themes/{name}.json`.
/// Returns None if the file doesn't exist or fails to parse.
pub fn load_custom_theme(name: &str) -> Option<Theme> {
    let path = themes_dir().join(format!("{name}.json"));
    let content = fs::read_to_string(&path).ok()?;
    let tf: ThemeFile = serde_json::from_str(&content).ok()?;
    Some(tf.apply_to(Theme::dark()))
}

/// Discover all custom theme names from the themes directory.
/// Returns sorted list of theme names (without .json extension).
pub fn discover_custom_themes() -> Option<Vec<ThemeName>> {
    let dir = themes_dir();
    let entries = fs::read_dir(&dir).ok()?;
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                path.file_stem()?.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    Some(names.into_iter().map(ThemeName::Custom).collect())
}

impl ThemeFile {
    /// Apply this theme file on top of a base theme, overriding only specified fields.
    fn apply_to(&self, base: Theme) -> Theme {
        Theme {
            sidebar_bg: self
                .sidebar_bg
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_bg),
            sidebar_title: self
                .sidebar_title
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_title),
            sidebar_text: self
                .sidebar_text
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_text),
            sidebar_dim: self
                .sidebar_dim
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_dim),
            sidebar_highlight: self
                .sidebar_highlight
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_highlight),
            sidebar_selected: self
                .sidebar_selected
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.sidebar_selected),

            chat_border: self
                .chat_border
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.chat_border),
            chat_title: self
                .chat_title
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.chat_title),

            agent_claude: self
                .agent_claude
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.agent_claude),
            agent_codex: self
                .agent_codex
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.agent_codex),
            agent_omp: self
                .agent_omp
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.agent_omp),

            status_running: self
                .status_running
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.status_running),
            status_done: self
                .status_done
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.status_done),
            status_error: self
                .status_error
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.status_error),

            popup_border: self
                .popup_border
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.popup_border),
            popup_title: self
                .popup_title
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.popup_title),
            popup_text: self
                .popup_text
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.popup_text),
            popup_hint: self
                .popup_hint
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.popup_hint),

            accent: self
                .accent
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.accent),
            dim: self
                .dim
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.dim),
            bold_text: self
                .bold_text
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.bold_text),
            input_cursor: self
                .input_cursor
                .as_deref()
                .and_then(parse_color)
                .unwrap_or(base.input_cursor),
        }
    }
}

/// Named color slots used across the TUI.
/// Each slot maps to a specific UI element.
#[derive(Clone, Debug)]
pub struct Theme {
    // Sidebar
    pub sidebar_bg: Color,
    pub sidebar_title: Color,
    pub sidebar_text: Color,
    pub sidebar_dim: Color,
    pub sidebar_highlight: Color,
    pub sidebar_selected: Color,

    // Chat
    pub chat_border: Color,
    pub chat_title: Color,

    // Agent colors
    pub agent_claude: Color,
    pub agent_codex: Color,
    pub agent_omp: Color,

    // Status
    pub status_running: Color,
    pub status_done: Color,
    pub status_error: Color,

    // Popups
    pub popup_border: Color,
    pub popup_title: Color,
    pub popup_text: Color,
    pub popup_hint: Color,

    // General
    pub accent: Color,
    pub dim: Color,
    pub bold_text: Color,
    pub input_cursor: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            sidebar_bg: Color::Reset,
            sidebar_title: Color::Cyan,
            sidebar_text: Color::White,
            sidebar_dim: Color::DarkGray,
            sidebar_highlight: Color::Yellow,
            sidebar_selected: Color::Cyan,

            chat_border: Color::DarkGray,
            chat_title: Color::Cyan,

            agent_claude: Color::Cyan,
            agent_codex: Color::Green,
            agent_omp: Color::Blue,

            status_running: Color::Yellow,
            status_done: Color::Green,
            status_error: Color::Red,

            popup_border: Color::Cyan,
            popup_title: Color::Cyan,
            popup_text: Color::White,
            popup_hint: Color::Yellow,

            accent: Color::Cyan,
            dim: Color::DarkGray,
            bold_text: Color::White,
            input_cursor: Color::Gray,
        }
    }

    pub fn light() -> Self {
        Theme {
            sidebar_bg: Color::Reset,
            sidebar_title: Color::Blue,
            sidebar_text: Color::Black,
            sidebar_dim: Color::Gray,
            sidebar_highlight: Color::Red,
            sidebar_selected: Color::Blue,

            chat_border: Color::Gray,
            chat_title: Color::Blue,

            agent_claude: Color::Blue,
            agent_codex: Color::Green,
            agent_omp: Color::Cyan,

            status_running: Color::Yellow,
            status_done: Color::Green,
            status_error: Color::Red,

            popup_border: Color::Blue,
            popup_title: Color::Blue,
            popup_text: Color::Black,
            popup_hint: Color::Yellow,

            accent: Color::Blue,
            dim: Color::Gray,
            bold_text: Color::Black,
            input_cursor: Color::Gray,
        }
    }

    /// Catppuccin Mocha — warm, soft pastels on dark blue-grey backgrounds.
    /// https://catppuccin.com/palette/mocha
    pub fn mocha() -> Self {
        Theme {
            // Surface colours
            sidebar_bg: Color::Rgb(0x18, 0x18, 0x25),   // Mantle
            sidebar_title: Color::Rgb(0xba, 0xc2, 0xde),  // Subtext1
            sidebar_text: Color::Rgb(0xcd, 0xd6, 0xf4),   // Text
            sidebar_dim: Color::Rgb(0x6c, 0x70, 0x86),    // Overlay0
            sidebar_highlight: Color::Rgb(0xf9, 0xe2, 0xaf), // Yellow
            sidebar_selected: Color::Rgb(0x89, 0xb4, 0xfa), // Blue

            chat_border: Color::Rgb(0x31, 0x32, 0x44),    // Surface0
            chat_title: Color::Rgb(0xb4, 0xbe, 0xfe),     // Lavender

            agent_claude: Color::Rgb(0x89, 0xb4, 0xfa),   // Blue
            agent_codex: Color::Rgb(0x94, 0xe2, 0xd5),    // Teal
            agent_omp: Color::Rgb(0xcb, 0xa6, 0xf7),     // Mauve

            status_running: Color::Rgb(0xf9, 0xe2, 0xaf), // Yellow
            status_done: Color::Rgb(0x94, 0xe2, 0xd5),    // Teal
            status_error: Color::Rgb(0xf3, 0x8b, 0xa8),  // Red

            popup_border: Color::Rgb(0xb4, 0xbe, 0xfe),   // Lavender
            popup_title: Color::Rgb(0xb4, 0xbe, 0xfe),    // Lavender
            popup_text: Color::Rgb(0xcd, 0xd6, 0xf4),     // Text
            popup_hint: Color::Rgb(0xf9, 0xe2, 0xaf),     // Yellow

            accent: Color::Rgb(0xb4, 0xbe, 0xfe),         // Lavender
            dim: Color::Rgb(0x6c, 0x70, 0x86),            // Overlay0
            bold_text: Color::Rgb(0xcd, 0xd6, 0xf4),      // Text
            input_cursor: Color::Rgb(0x45, 0x47, 0x5a),   // Surface1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_named_colors() {
        assert_eq!(parse_color("Cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("Red"), Some(Color::Red));
        assert_eq!(parse_color("DarkGray"), Some(Color::DarkGray));
        assert_eq!(parse_color("LightBlue"), Some(Color::LightBlue));
        assert_eq!(parse_color("White"), Some(Color::White));
        assert_eq!(parse_color("Reset"), Some(Color::Reset));
    }

    #[test]
    fn test_parse_hex_colors() {
        assert_eq!(parse_color("#00ffff"), Some(Color::Rgb(0, 255, 255)));
        assert_eq!(parse_color("#1a1a2e"), Some(Color::Rgb(0x1a, 0x1a, 0x2e)));
        assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
        assert_eq!(parse_color("#ffffff"), Some(Color::Rgb(255, 255, 255)));
    }

    #[test]
    fn test_parse_rgb_format() {
        assert_eq!(parse_color("Rgb(0,255,255)"), Some(Color::Rgb(0, 255, 255)));
        assert_eq!(
            parse_color("Rgb(128, 64, 32)"),
            Some(Color::Rgb(128, 64, 32))
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_color("invalid"), None);
        assert_eq!(parse_color("#xyz"), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("Rgb(a,b,c)"), None);
    }

    #[test]
    fn test_theme_name_custom_label() {
        let name = ThemeName::Custom("ocean".into());
        assert_eq!(name.label(), "ocean");
    }

    #[test]
    fn test_theme_name_cycle_basic() {
        // Without custom themes, Dark -> Light -> Dark
        assert_eq!(ThemeName::Dark.cycle(), ThemeName::Light);
    }

    #[test]
    fn test_theme_file_apply_override() {
        let tf = ThemeFile {
            sidebar_title: Some("#ff0000".into()),
            accent: Some("Green".into()),
            ..ThemeFile::default()
        };
        let theme = tf.apply_to(Theme::dark());
        assert_eq!(theme.sidebar_title, Color::Rgb(255, 0, 0));
        assert_eq!(theme.accent, Color::Green);
        // Other fields unchanged from Dark
        assert_eq!(theme.sidebar_text, Color::White);
    }
}
