use std::path::PathBuf;

use serde::Deserialize;

/// TUI configuration, loaded from `~/.config/rustify/tui.toml`.
#[derive(Debug, Deserialize)]
pub struct TuiConfig {
    /// Directories to scan for music files.
    #[serde(default)]
    pub music_dirs: Vec<PathBuf>,

    /// ALSA device name passed to rustify-core.
    #[serde(default = "default_alsa_device")]
    pub alsa_device: String,

    /// Theme preset name.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Custom theme color overrides (optional).
    #[serde(default)]
    pub custom_theme: Option<CustomThemeConfig>,

    #[serde(default)]
    #[allow(dead_code)]
    pub replay_gain: bool,

    #[serde(default)]
    pub crossfade_ms: u64,

    #[serde(default)]
    pub listenbrainz_token: String,
}

/// Custom theme colors as hex strings (#RRGGBB).
#[derive(Debug, Default, Deserialize)]
pub struct CustomThemeConfig {
    pub fg: Option<String>,
    pub fg_dim: Option<String>,
    pub accent: Option<String>,
    pub accent_dim: Option<String>,
    pub border: Option<String>,
    pub error: Option<String>,
    pub visualizer: Option<Vec<String>>,
}

fn default_alsa_device() -> String {
    "default".to_string()
}

fn default_theme() -> String {
    "default".to_string()
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            music_dirs: Vec::new(),
            alsa_device: default_alsa_device(),
            theme: default_theme(),
            custom_theme: None,
            replay_gain: false,
            crossfade_ms: 0,
            listenbrainz_token: String::new(),
        }
    }
}

impl TuiConfig {
    /// Platform-appropriate config file path.
    /// Linux/macOS: `~/.config/rustify/tui.toml`
    /// Windows: `%APPDATA%\rustify\tui.toml`
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("rustify").join("tui.toml"))
    }

    /// Load config from disk. Returns defaults if file doesn't exist.
    /// Prints a warning to stderr if the file exists but can't be parsed.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("rustify: failed to parse {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let config = TuiConfig::default();
        assert_eq!(config.alsa_device, "default");
        assert!(config.music_dirs.is_empty());
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn parse_from_toml_string() {
        let toml_str = r#"
            music_dirs = ["/home/pi/Music"]
            alsa_device = "hw:0"
            theme = "nord"
        "#;
        let config: TuiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.music_dirs,
            vec![std::path::PathBuf::from("/home/pi/Music")]
        );
        assert_eq!(config.alsa_device, "hw:0");
        assert_eq!(config.theme, "nord");
    }

    #[test]
    fn parse_partial_toml_uses_defaults() {
        let toml_str = r#"
            music_dirs = ["/Music"]
        "#;
        let config: TuiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.alsa_device, "default");
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn config_path_returns_some() {
        let _ = TuiConfig::config_path();
    }
}
