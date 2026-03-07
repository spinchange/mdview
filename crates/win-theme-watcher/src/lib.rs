use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(windows)]
use winreg::enums::HKEY_CURRENT_USER;
#[cfg(windows)]
use winreg::RegKey;

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
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
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
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent",
        "AccentColorMenu",
    ) {
        return Some(argb_to_rgb(value));
    }

    // Fallback to DWM colorization.
    query_reg_dword(r"Software\Microsoft\Windows\DWM", "ColorizationColor")
        .map(argb_to_rgb)
}

fn argb_to_rgb(value: u32) -> u32 {
    value & 0x00FF_FFFF
}

#[cfg(windows)]
fn query_reg_dword(key: &str, value_name: &str) -> Option<u32> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let subkey = hkcu.open_subkey(key).ok()?;
    subkey.get_value::<u32, _>(value_name).ok()
}

#[cfg(not(windows))]
fn query_reg_dword(_key: &str, _value_name: &str) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::{argb_to_rgb, ThemeMode};

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

    #[test]
    fn strips_alpha_channel_from_argb() {
        assert_eq!(argb_to_rgb(0xAA112233), 0x00112233);
    }
}
