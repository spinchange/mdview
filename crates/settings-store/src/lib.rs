use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use win_theme_watcher::{ThemeMode, ThemeSnapshot};

const DEFAULT_CONFIG_RELATIVE_PATH: &str = "mdview\\config.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub theme: ThemeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub mode: ThemeModePreference,
    #[serde(default)]
    pub accent_hex: Option<String>,
    #[serde(default)]
    pub accent_rgb: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeModePreference {
    #[default]
    System,
    Light,
    Dark,
}

pub fn default_config_path() -> PathBuf {
    if let Ok(appdata) = env::var("APPDATA") {
        return PathBuf::from(appdata).join(DEFAULT_CONFIG_RELATIVE_PATH);
    }

    if let Ok(home) = env::var("USERPROFILE") {
        return PathBuf::from(home).join("AppData\\Roaming").join(DEFAULT_CONFIG_RELATIVE_PATH);
    }

    PathBuf::from(DEFAULT_CONFIG_RELATIVE_PATH)
}

pub fn load_config() -> Option<AppConfig> {
    let path = default_config_path();
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<AppConfig>(&raw).ok()
}

pub fn resolve_theme_snapshot(system: ThemeSnapshot) -> ThemeSnapshot {
    let Some(config) = load_config() else {
        return system;
    };

    let mut resolved = system;
    resolved.mode = match config.theme.mode {
        ThemeModePreference::System => resolved.mode,
        ThemeModePreference::Light => ThemeMode::Light,
        ThemeModePreference::Dark => ThemeMode::Dark,
    };

    if let Some(accent) = parse_accent_rgb(&config.theme) {
        resolved.accent_rgb = accent;
    }

    resolved
}

fn parse_accent_rgb(theme: &ThemeConfig) -> Option<u32> {
    if let Some(hex) = theme.accent_hex.as_deref() {
        return parse_hex_rgb(hex);
    }
    theme.accent_rgb.map(|value| value & 0x00FF_FFFF)
}

fn parse_hex_rgb(value: &str) -> Option<u32> {
    let normalized = value.trim().trim_start_matches('#');
    if normalized.len() != 6 {
        return None;
    }

    u32::from_str_radix(normalized, 16)
        .ok()
        .map(|rgb| rgb & 0x00FF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::parse_hex_rgb;

    #[test]
    fn parses_hex_accent() {
        assert_eq!(parse_hex_rgb("#112233"), Some(0x112233));
        assert_eq!(parse_hex_rgb("AABBCC"), Some(0xAABBCC));
        assert_eq!(parse_hex_rgb("#XYZ123"), None);
    }
}
