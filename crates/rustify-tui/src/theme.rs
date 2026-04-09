use ratatui::style::Color;
use serde::Deserialize;

use crate::config::TuiConfig;

/// Color theme for the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub fg: Color,
    pub fg_dim: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub border: Color,
    pub error: Color,
    pub visualizer: Vec<Color>,
}

impl Theme {
    pub fn default_theme() -> Self {
        Self {
            name: "default".into(),
            fg: Color::White,
            fg_dim: Color::Gray,
            accent: Color::Magenta,
            accent_dim: Color::DarkGray,
            border: Color::DarkGray,
            error: Color::Yellow,
            visualizer: vec![Color::DarkGray, Color::Magenta],
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "nord".into(),
            fg: Color::Rgb(216, 222, 233),       // #D8DEE9
            fg_dim: Color::Rgb(76, 86, 106),     // #4C566A
            accent: Color::Rgb(136, 192, 208),   // #88C0D0
            accent_dim: Color::Rgb(94, 129, 172), // #5E81AC
            border: Color::Rgb(59, 66, 82),      // #3B4252
            error: Color::Rgb(191, 97, 106),     // #BF616A
            visualizer: vec![
                Color::Rgb(94, 129, 172),  // #5E81AC
                Color::Rgb(136, 192, 208), // #88C0D0
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "dracula".into(),
            fg: Color::Rgb(248, 248, 242),       // #F8F8F2
            fg_dim: Color::Rgb(98, 114, 164),    // #6272A4
            accent: Color::Rgb(189, 147, 249),   // #BD93F9
            accent_dim: Color::Rgb(139, 97, 199), // dimmer purple
            border: Color::Rgb(68, 71, 90),      // #44475A
            error: Color::Rgb(255, 85, 85),      // #FF5555
            visualizer: vec![
                Color::Rgb(98, 114, 164),  // #6272A4
                Color::Rgb(189, 147, 249), // #BD93F9
            ],
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".into(),
            fg: Color::Rgb(235, 219, 178),       // #EBDBB2
            fg_dim: Color::Rgb(146, 131, 116),   // #928374
            accent: Color::Rgb(250, 189, 47),    // #FABD2F
            accent_dim: Color::Rgb(215, 153, 33), // #D79921
            border: Color::Rgb(60, 56, 54),      // #3C3836
            error: Color::Rgb(251, 73, 52),      // #FB4934
            visualizer: vec![
                Color::Rgb(104, 157, 106), // #689D6A
                Color::Rgb(250, 189, 47),  // #FABD2F
            ],
        }
    }

    pub fn catppuccin() -> Self {
        Self {
            name: "catppuccin".into(),
            fg: Color::Rgb(205, 214, 244),       // #CDD6F4
            fg_dim: Color::Rgb(88, 91, 112),     // #585B70
            accent: Color::Rgb(203, 166, 247),   // #CBA6F7
            accent_dim: Color::Rgb(147, 110, 191), // dimmer mauve
            border: Color::Rgb(49, 50, 68),      // #313244
            error: Color::Rgb(243, 139, 168),    // #F38BA8
            visualizer: vec![
                Color::Rgb(88, 91, 112),   // #585B70
                Color::Rgb(203, 166, 247), // #CBA6F7
            ],
        }
    }

    /// Load a theme by preset name.
    pub fn from_name(name: &str) -> Self {
        match name {
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            "gruvbox" => Self::gruvbox(),
            "catppuccin" => Self::catppuccin(),
            _ => Self::default_theme(),
        }
    }

    /// Load theme from config — resolves preset or custom theme.
    pub fn from_config(config: &TuiConfig) -> Self {
        let mut theme = Self::from_name(&config.theme);

        // Apply custom overrides if present
        if let Some(ref custom) = config.custom_theme {
            if let Some(ref hex) = custom.fg {
                if let Some(c) = parse_hex_color(hex) {
                    theme.fg = c;
                }
            }
            if let Some(ref hex) = custom.fg_dim {
                if let Some(c) = parse_hex_color(hex) {
                    theme.fg_dim = c;
                }
            }
            if let Some(ref hex) = custom.accent {
                if let Some(c) = parse_hex_color(hex) {
                    theme.accent = c;
                }
            }
            if let Some(ref hex) = custom.accent_dim {
                if let Some(c) = parse_hex_color(hex) {
                    theme.accent_dim = c;
                }
            }
            if let Some(ref hex) = custom.border {
                if let Some(c) = parse_hex_color(hex) {
                    theme.border = c;
                }
            }
            if let Some(ref hex) = custom.error {
                if let Some(c) = parse_hex_color(hex) {
                    theme.error = c;
                }
            }
            if let Some(ref colors) = custom.visualizer {
                let parsed: Vec<Color> = colors
                    .iter()
                    .filter_map(|h| parse_hex_color(h))
                    .collect();
                if !parsed.is_empty() {
                    theme.visualizer = parsed;
                }
            }
            theme.name = "custom".into();
        }

        theme
    }
}

/// Parse a `#RRGGBB` hex string to a ratatui Color.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_magenta_accent() {
        let theme = Theme::default_theme();
        assert_eq!(theme.accent, Color::Magenta);
    }

    #[test]
    fn all_presets_load() {
        let names = ["default", "nord", "dracula", "gruvbox", "catppuccin"];
        for name in names {
            let theme = Theme::from_name(name);
            assert!(!theme.name.is_empty());
        }
    }

    #[test]
    fn unknown_name_falls_back_to_default() {
        let theme = Theme::from_name("nonexistent");
        assert_eq!(theme.name, "default");
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#FF00FF"), Some(Color::Rgb(255, 0, 255)));
        assert_eq!(parse_hex_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color("not-a-color"), None);
        assert_eq!(parse_hex_color("#GG00FF"), None);
    }
}
