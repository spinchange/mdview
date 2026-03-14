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
    watch_loop_with_emit(
        stop_rx,
        event_rx,
        target_name,
        Duration::from_millis(WATCH_DEBOUNCE_MS),
        || {
            let _ = app.emit(FILE_CHANGED_EVENT, ());
        },
    );
}

fn watch_loop_with_emit<F>(
    stop_rx: Receiver<()>,
    event_rx: Receiver<notify::Result<Event>>,
    target_name: std::ffi::OsString,
    debounce: Duration,
    mut emit_changed: F,
) where
    F: FnMut(),
{
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
            emit_changed();
            pending = false;
        }
    }
}

fn event_targets_file(event: &Event, target_name: &std::ffi::OsString) -> bool {
    event.paths.iter().any(|path| path.file_name() == Some(target_name.as_os_str()))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use notify::{event::ModifyKind, Event, EventKind};

    use super::{event_targets_file, watch_loop_with_emit};

    fn changed_event(path: &str) -> notify::Result<Event> {
        Ok(Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: vec![path.into()],
            attrs: Default::default(),
        })
    }

    #[test]
    fn matches_events_for_target_file_name() {
        let event = changed_event("C:\\notes\\sample.md").expect("notify event");
        assert!(event_targets_file(&event, &OsString::from("sample.md")));
    }

    #[test]
    fn ignores_events_for_temp_or_other_files() {
        let event = changed_event("C:\\notes\\.sample.md.mdview-123.tmp").expect("notify event");
        assert!(!event_targets_file(&event, &OsString::from("sample.md")));
    }

    #[test]
    fn debounces_multiple_target_events_into_single_emit() {
        let (event_tx, event_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let emit_count = Arc::new(AtomicUsize::new(0));
        let emit_count_for_thread = Arc::clone(&emit_count);

        let worker = thread::spawn(move || {
            watch_loop_with_emit(
                stop_rx,
                event_rx,
                OsString::from("sample.md"),
                Duration::from_millis(20),
                move || {
                    emit_count_for_thread.fetch_add(1, Ordering::SeqCst);
                },
            );
        });

        event_tx
            .send(changed_event("C:\\notes\\sample.md"))
            .expect("send first event");
        event_tx
            .send(changed_event("C:\\notes\\sample.md"))
            .expect("send second event");
        event_tx
            .send(changed_event("C:\\notes\\other.md"))
            .expect("send unrelated event");

        thread::sleep(Duration::from_millis(80));
        stop_tx.send(()).expect("stop watcher");
        worker.join().expect("join watcher");

        assert_eq!(emit_count.load(Ordering::SeqCst), 1);
    }
}
