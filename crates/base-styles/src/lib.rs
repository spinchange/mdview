use win_theme_watcher::{ThemeMode, ThemeSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeTokens {
    pub mode: ThemeMode,
    pub accent: String,
    pub bg: String,
    pub surface: String,
    pub text: String,
    pub muted_text: String,
    pub border: String,
    pub code_bg: String,
}

impl ThemeTokens {
    pub fn to_css_vars(&self) -> String {
        format!(
            ":root {{\n  --mdv-accent: {};\n  --mdv-bg: {};\n  --mdv-surface: {};\n  --mdv-text: {};\n  --mdv-muted-text: {};\n  --mdv-border: {};\n  --mdv-code-bg: {};\n}}\n",
            self.accent,
            self.bg,
            self.surface,
            self.text,
            self.muted_text,
            self.border,
            self.code_bg
        )
    }
}

pub fn tokens_from_snapshot(snapshot: &ThemeSnapshot) -> ThemeTokens {
    let accent = snapshot.accent_hex();

    match snapshot.mode {
        ThemeMode::Dark => ThemeTokens {
            mode: ThemeMode::Dark,
            accent,
            bg: "#1E1E1E".to_string(),
            surface: "#252526".to_string(),
            text: "#F3F3F3".to_string(),
            muted_text: "#B6B6B6".to_string(),
            border: "#3C3C3C".to_string(),
            code_bg: "#2D2D2D".to_string(),
        },
        ThemeMode::Light => ThemeTokens {
            mode: ThemeMode::Light,
            accent,
            bg: "#FFFFFF".to_string(),
            surface: "#F7F7F8".to_string(),
            text: "#1F1F1F".to_string(),
            muted_text: "#5D5D5D".to_string(),
            border: "#D9D9DD".to_string(),
            code_bg: "#F3F3F3".to_string(),
        },
    }
}

pub fn initial_shell_background(snapshot: &ThemeSnapshot) -> &'static str {
    match snapshot.mode {
        ThemeMode::Dark => "#1E1E1E",
        ThemeMode::Light => "#FFFFFF",
    }
}

#[cfg(test)]
mod tests {
    use super::{initial_shell_background, tokens_from_snapshot};
    use win_theme_watcher::{ThemeMode, ThemeSnapshot};

    #[test]
    fn dark_tokens_have_dark_background() {
        let snapshot = ThemeSnapshot {
            mode: ThemeMode::Dark,
            accent_rgb: 0x0055AA,
        };
        let tokens = tokens_from_snapshot(&snapshot);
        assert_eq!(tokens.bg, "#1E1E1E");
        assert_eq!(initial_shell_background(&snapshot), "#1E1E1E");
    }

    #[test]
    fn css_vars_include_accent() {
        let snapshot = ThemeSnapshot {
            mode: ThemeMode::Light,
            accent_rgb: 0x00112233,
        };
        let tokens = tokens_from_snapshot(&snapshot);
        let css = tokens.to_css_vars();
        assert!(css.contains("--mdv-accent: #112233"));
    }
}
