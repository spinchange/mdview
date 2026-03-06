use std::time::Duration;

use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN_WINDOW_LABEL: &str = "main";
pub const STARTUP_TIMEOUT_MS: u64 = 500;

#[tauri::command]
pub fn window_ready(window: WebviewWindow) -> Result<(), String> {
    reveal_window(&window)
}

pub fn build_hidden_main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::App("index.html".into()))
        .visible(false)
        .title("mdview")
        .build()
}

pub fn arm_startup_timeout(window: WebviewWindow, timeout: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        let visible = window.is_visible().unwrap_or(false);
        if !visible {
            let _ = reveal_window(&window);
        }
    });
}

pub fn reveal_window(window: &WebviewWindow) -> Result<(), String> {
    let visible = window.is_visible().map_err(|e| e.to_string())?;
    if visible {
        return Ok(());
    }

    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
