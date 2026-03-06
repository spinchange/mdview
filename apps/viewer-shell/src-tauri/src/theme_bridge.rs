use std::time::Duration;

use base_styles::{tokens_from_snapshot, ThemeTokens};
use win_theme_watcher::{current_snapshot, ThemeWatcher};

pub const THEME_EVENT_NAME: &str = "mdview://theme-updated";
pub const DEFAULT_THEME_POLL_MS: u64 = 750;

pub fn initial_tokens() -> ThemeTokens {
    tokens_from_snapshot(&current_snapshot())
}

pub fn start_theme_sync<F>(poll_interval: Duration, emit: F) -> ThemeWatcher
where
    F: Fn(ThemeTokens) + Send + 'static,
{
    ThemeWatcher::start(poll_interval, move |snapshot| {
        let tokens = tokens_from_snapshot(&snapshot);
        emit(tokens);
    })
}
