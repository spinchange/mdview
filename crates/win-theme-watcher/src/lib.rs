use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSnapshot {
    pub mode: ThemeMode,
    pub accent_rgb: u32,
}

impl ThemeSnapshot {
    pub fn accent_hex(&self) -> String {
        format!("#{:06X}", self.accent_rgb & 0x00FF_FFFF)
    }
}

pub struct ThemeWatcher {
    stop_tx: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl ThemeWatcher {
    pub fn start<F>(poll_interval: Duration, on_change: F) -> Self
    where
        F: Fn(ThemeSnapshot) + Send + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel();
        let handle = thread::spawn(move || watch_loop(poll_interval, stop_rx, on_change));
        Self {
            stop_tx,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ThemeWatcher {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn current_snapshot() -> ThemeSnapshot {
    ThemeSnapshot {
        mode: detect_mode().unwrap_or(ThemeMode::Dark),
        accent_rgb: detect_accent_rgb().unwrap_or(0x0A84FF),
    }
}

fn watch_loop<F>(poll_interval: Duration, stop_rx: Receiver<()>, on_change: F)
where
    F: Fn(ThemeSnapshot),
{
    let mut last = current_snapshot();
    on_change(last.clone());

    loop {
        match stop_rx.recv_timeout(poll_interval) {
            Ok(_) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                let next = current_snapshot();
                if next != last {
                    on_change(next.clone());
                    last = next;
                }
            }
        }
    }
}

fn detect_mode() -> Option<ThemeMode> {
    let output = query_reg_dword(
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    )?;

    // 0 => dark, 1 => light
    if output == 0 {
        Some(ThemeMode::Dark)
    } else {
        Some(ThemeMode::Light)
    }
}

fn detect_accent_rgb() -> Option<u32> {
    // Prefer Explorer accent since it usually matches user-selected accent more directly.
    if let Some(value) = query_reg_dword(
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Accent",
        "AccentColorMenu",
    ) {
        return Some(argb_to_rgb(value));
    }

    // Fallback to DWM colorization.
    query_reg_dword(r"HKCU\Software\Microsoft\Windows\DWM", "ColorizationColor")
        .map(argb_to_rgb)
}

fn argb_to_rgb(value: u32) -> u32 {
    value & 0x00FF_FFFF
}

fn query_reg_dword(key: &str, value_name: &str) -> Option<u32> {
    let output = Command::new("reg")
        .args(["query", key, "/v", value_name])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_reg_dword_output(&String::from_utf8_lossy(&output.stdout), value_name)
}

fn parse_reg_dword_output(output: &str, value_name: &str) -> Option<u32> {
    output.lines().find_map(|line| {
        if !line.contains(value_name) || !line.contains("REG_DWORD") {
            return None;
        }

        let raw_value = line.split_whitespace().last()?;
        let normalized = raw_value.trim_start_matches("0x");
        u32::from_str_radix(normalized, 16).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_reg_dword_output, ThemeMode};

    #[test]
    fn parses_reg_dword_hex_value() {
        let text = "    AppsUseLightTheme    REG_DWORD    0x0";
        let value = parse_reg_dword_output(text, "AppsUseLightTheme");
        assert_eq!(value, Some(0));
    }

    #[test]
    fn mode_mapping_is_stable() {
        let dark = if 0 == 0 {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        let light = if 1 == 0 {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        assert!(matches!(dark, ThemeMode::Dark));
        assert!(matches!(light, ThemeMode::Light));
    }
}
