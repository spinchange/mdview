#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod file_open;
mod file_watch;
mod theme_bridge;
mod window_boot;

use std::sync::Mutex;
use std::time::Duration;
use std::{env, process};

use md_engine::{MarkdownEngine, RenderedDocument};
use tauri::{Emitter, Manager, RunEvent};
use win_theme_watcher::ThemeWatcher;

use crate::file_watch::FileWatcherState;

struct ThemeWatcherState(Mutex<Option<ThemeWatcher>>);

#[tauri::command]
fn get_initial_theme_css() -> String {
    theme_bridge::initial_tokens().to_css_vars()
}

#[tauri::command]
fn render_markdown(markdown: String) -> RenderedDocument {
    MarkdownEngine::default().render(&markdown)
}

fn shutdown_background_services(app_handle: &tauri::AppHandle) {
    if let Some(file_state) = app_handle.try_state::<FileWatcherState>() {
        match file_state.0.lock() {
            Ok(mut guard) => {
                if let Some(handle) = guard.take() {
                    handle.stop();
                }
            }
            Err(_) => {
                eprintln!("[mdview] failed to lock file watcher state during shutdown");
            }
        };
    }

    if let Some(theme_state) = app_handle.try_state::<ThemeWatcherState>() {
        match theme_state.0.lock() {
            Ok(mut guard) => {
                if let Some(watcher) = guard.take() {
                    watcher.stop();
                }
            }
            Err(_) => {
                eprintln!("[mdview] failed to lock theme watcher state during shutdown");
            }
        };
    }
}

fn maybe_handle_shell_registration_args() -> bool {
    let args: Vec<String> = env::args().skip(1).collect();
    let wants_register = args.iter().any(|arg| arg == "--register");
    let wants_unregister = args.iter().any(|arg| arg == "--unregister");

    if !wants_register && !wants_unregister {
        return false;
    }

    if wants_register && wants_unregister {
        eprintln!("[mdview] invalid arguments: choose either --register or --unregister.");
        process::exit(2);
    }

    let result = if wants_register {
        win_installer::register_all()
    } else {
        win_installer::unregister_all()
    };

    match result {
        Ok(_) => {
            if wants_register {
                println!("[mdview] Windows shell integration registered successfully.");
                println!(
                    "[mdview] Preview handler and context menu are now active for .md/.markdown files."
                );
            } else {
                println!("[mdview] Windows shell integration removed successfully.");
            }
            process::exit(0);
        }
        Err(err) => {
            eprintln!("[mdview] Windows shell integration command failed: {err}");
            process::exit(1);
        }
    }
}

fn main() {
    if maybe_handle_shell_registration_args() {
        return;
    }

    let app = tauri::Builder::default()
        .setup(|app| {
            let launch_path = file_open::detect_launch_path_from_args(std::env::args_os());
            app.manage(file_open::LaunchPathState::new(launch_path));
            app.manage(FileWatcherState(Mutex::new(None)));
            app.manage(ThemeWatcherState(Mutex::new(None)));

            let launch_state = app.state::<file_open::LaunchPathState>();
            if let Some(path) = launch_state.path_clone() {
                match file_watch::spawn_launch_file_watcher(app.handle().clone(), path) {
                    Ok(handle) => {
                        let watcher_state = app.state::<FileWatcherState>();
                        match watcher_state.0.lock() {
                            Ok(mut guard) => {
                                *guard = Some(handle);
                            }
                            Err(_) => {
                                eprintln!("[mdview] failed to store file watcher handle");
                            }
                        };
                    }
                    Err(err) => {
                        eprintln!("[mdview] file watcher unavailable: {err}");
                    }
                }
            }

            let window = window_boot::build_hidden_main_window(app.handle())?;
            window_boot::arm_startup_timeout(
                window,
                Duration::from_millis(window_boot::STARTUP_TIMEOUT_MS),
            );

            let app_handle = app.handle().clone();
            let watcher = theme_bridge::start_theme_sync(
                Duration::from_millis(theme_bridge::DEFAULT_THEME_POLL_MS),
                move |tokens| {
                    let _ = app_handle.emit(theme_bridge::THEME_EVENT_NAME, tokens.to_css_vars());
                },
            );

            let state = app.state::<ThemeWatcherState>();
            if let Ok(mut guard) = state.0.lock() {
                *guard = Some(watcher);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            file_open::get_launch_path,
            file_open::read_launch_markdown,
            file_open::read_markdown_file,
            window_boot::window_ready,
            get_initial_theme_css,
            render_markdown
        ])
        .build(tauri::generate_context!())
        .expect("failed to build mdview shell");

    app.run(|app_handle, event| match event {
        RunEvent::Exit | RunEvent::ExitRequested { .. } => {
            shutdown_background_services(app_handle);
        }
        _ => {}
    });
}
