use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

pub const FILE_CHANGED_EVENT: &str = "mdview://file-changed";
pub const WATCH_DEBOUNCE_MS: u64 = 100;

pub struct FileWatcherState(pub Mutex<Option<FileWatcherHandle>>);

pub struct FileWatcherHandle {
    stop_tx: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl FileWatcherHandle {
    pub fn stop(mut self) {
        let _ = self.stop_tx.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for FileWatcherHandle {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_launch_file_watcher(app: AppHandle, target: PathBuf) -> Result<FileWatcherHandle, String> {
    let watch_root = match target.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(parent) => parent.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| format!("failed to resolve current directory for watcher: {e}"))?,
    };

    let target_name = target
        .file_name()
        .map(|name| name.to_os_string())
        .ok_or_else(|| format!("cannot resolve file name: {}", target.display()))?;

    let (event_tx, event_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();

    let worker = thread::spawn(move || {
        let mut watcher = match build_watcher(event_tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                eprintln!("[mdview] failed to create file watcher: {err}");
                return;
            }
        };

        if let Err(err) = watcher.watch(&watch_root, RecursiveMode::NonRecursive) {
            eprintln!(
                "[mdview] failed to watch path {}: {}",
                watch_root.display(),
                err
            );
            return;
        }

        watch_loop(app, stop_rx, event_rx, target_name);
    });

    Ok(FileWatcherHandle {
        stop_tx,
        worker: Some(worker),
    })
}

fn build_watcher(event_tx: Sender<notify::Result<Event>>) -> Result<RecommendedWatcher, notify::Error> {
    RecommendedWatcher::new(
        move |event| {
            let _ = event_tx.send(event);
        },
        Config::default(),
    )
}

fn watch_loop(
    app: AppHandle,
    stop_rx: Receiver<()>,
    event_rx: Receiver<notify::Result<Event>>,
    target_name: std::ffi::OsString,
) {
    let debounce = Duration::from_millis(WATCH_DEBOUNCE_MS);
    let mut pending = false;
    let mut next_emit_at = Instant::now();

    loop {
        match stop_rx.try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        match event_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(event)) => {
                if event_targets_file(&event, &target_name) {
                    pending = true;
                    next_emit_at = Instant::now() + debounce;
                }
            }
            Ok(Err(err)) => {
                eprintln!("[mdview] file watcher event error: {err}");
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        if pending && Instant::now() >= next_emit_at {
            let _ = app.emit(FILE_CHANGED_EVENT, ());
            pending = false;
        }
    }
}

fn event_targets_file(event: &Event, target_name: &std::ffi::OsString) -> bool {
    event.paths.iter().any(|path| path.file_name() == Some(target_name.as_os_str()))
}
