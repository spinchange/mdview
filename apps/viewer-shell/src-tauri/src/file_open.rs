use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::file_watch;

pub struct LaunchPathState {
    launch_path: Mutex<Option<PathBuf>>,
}

impl LaunchPathState {
    pub fn new(launch_path: Option<PathBuf>) -> Self {
        Self {
            launch_path: Mutex::new(launch_path),
        }
    }

    pub fn as_string(&self) -> Option<String> {
        self.launch_path
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
    }

    pub fn path_clone(&self) -> Option<PathBuf> {
        self.launch_path.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn set_path(&self, path: Option<PathBuf>) -> Result<(), String> {
        let mut guard = self
            .launch_path
            .lock()
            .map_err(|_| "failed to lock launch path state".to_string())?;
        *guard = path;
        Ok(())
    }
}

pub fn detect_launch_path_from_args<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = OsString>,
{
    // argv[0] is executable path. Use the first non-flag argument as launch target.
    args.into_iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|candidate| {
            let is_flag = candidate
                .to_str()
                .map(|s| s.starts_with('-') || s.starts_with('/'))
                .unwrap_or(false);
            !is_flag
        })
}

#[tauri::command]
pub fn get_launch_path(state: State<'_, LaunchPathState>) -> Option<String> {
    state.as_string()
}

#[tauri::command]
pub fn read_launch_markdown(state: State<'_, LaunchPathState>) -> Result<Option<String>, String> {
    match state.path_clone() {
        Some(path) => read_markdown_file_impl(&path).map(Some),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn read_markdown_file(path: String) -> Result<String, String> {
    read_markdown_file_impl(Path::new(&path))
}

#[tauri::command]
pub fn write_launch_markdown(
    state: State<'_, LaunchPathState>,
    markdown: String,
) -> Result<(), String> {
    let path = state
        .path_clone()
        .ok_or_else(|| "no launch markdown file is active".to_string())?;
    write_markdown_file_impl(&path, &markdown)
}

#[tauri::command]
pub fn open_local_link(
    app: AppHandle,
    state: State<'_, LaunchPathState>,
    href: String,
) -> Result<OpenedLocalLink, String> {
    let target = resolve_local_link_target(state.path_clone().as_deref(), &href)?;
    let markdown = read_markdown_file_impl(&target)?;
    state.set_path(Some(target.clone()))?;
    file_watch::retarget_launch_file_watcher(&app, Some(target.clone()))?;
    Ok(OpenedLocalLink {
        path: target.to_string_lossy().to_string(),
        markdown,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenedLocalLink {
    pub path: String,
    pub markdown: String,
}

fn read_markdown_file_impl(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()));
    }

    if path.is_dir() {
        return Err(format!("path is a directory: {}", path.display()));
    }

    let mut file = open_shared_read(path).map_err(|e| format!("failed to open file: {e}"))?;
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| format!("failed to read utf-8 markdown: {e}"))?;
    Ok(content)
}

fn resolve_local_link_target(base_path: Option<&Path>, href: &str) -> Result<PathBuf, String> {
    let trimmed = href.trim();
    if trimmed.is_empty() {
        return Err("local link target is empty".to_string());
    }

    if trimmed.starts_with('#') {
        return Err("heading links are not local file targets".to_string());
    }

    let candidate = if let Some(rest) = trimmed.strip_prefix("mdview-local://") {
        PathBuf::from(decode_percent_escapes(rest)?.replace('/', "\\"))
    } else if let Some(rest) = trimmed.strip_prefix("file:///") {
        PathBuf::from(decode_percent_escapes(rest)?.replace('/', "\\"))
    } else if let Some(rest) = trimmed.strip_prefix("file://") {
        PathBuf::from(decode_percent_escapes(rest)?.replace('/', "\\"))
    } else {
        resolve_relative_link(base_path, &decode_percent_escapes(trimmed)?)?
    };

    if !candidate.exists() {
        return Err(format!("file not found: {}", candidate.display()));
    }

    if candidate.is_dir() {
        return Err(format!("path is a directory: {}", candidate.display()));
    }

    Ok(candidate)
}

fn resolve_relative_link(base_path: Option<&Path>, href: &str) -> Result<PathBuf, String> {
    let href_path = Path::new(href);
    if href_path.is_absolute() {
        return Ok(href_path.to_path_buf());
    }

    let base_path = base_path.ok_or_else(|| {
        "cannot resolve relative local link without an active launch file".to_string()
    })?;
    let parent = base_path.parent().ok_or_else(|| {
        format!(
            "cannot resolve relative local link from launch path: {}",
            base_path.display()
        )
    })?;

    Ok(parent.join(href_path))
}

fn decode_percent_escapes(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(format!("invalid percent-encoding in local link: {value}"));
            }

            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| format!("invalid percent-encoding in local link: {value}"))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| format!("invalid percent-encoding in local link: {value}"))?;
            decoded.push(byte);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(decoded)
        .map_err(|_| format!("local link is not valid UTF-8 after decoding: {value}"))
}

fn write_markdown_file_impl(path: &Path, content: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()));
    }

    if path.is_dir() {
        return Err(format!("path is a directory: {}", path.display()));
    }

    write_markdown_file_with(path, content, replace_file)
}

fn write_markdown_file_with<F>(path: &Path, content: &str, replace: F) -> Result<(), String>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let temp_path = create_temp_path(path);
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        replace_with_retry(&temp_path, path, replace)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result.map_err(|e| {
        format!(
            "failed to write markdown file: {}",
            describe_write_error(path, &e)
        )
    })
}

fn replace_with_retry<F>(
    temp_path: &Path,
    destination_path: &Path,
    mut replace: F,
) -> std::io::Result<()>
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    const REPLACE_RETRY_DELAYS_MS: [u64; 3] = [15, 40, 90];
    let mut last_error = None;

    for delay_ms in [0].into_iter().chain(REPLACE_RETRY_DELAYS_MS) {
        if delay_ms > 0 {
            thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

        match replace(temp_path, destination_path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                last_error = Some(err);
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error.unwrap_or_else(|| std::io::Error::from(ErrorKind::PermissionDenied)))
}

fn describe_write_error(path: &Path, error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::PermissionDenied {
        return format!(
            "{error}. The destination file may be locked by another app: {}",
            path.display()
        );
    }

    error.to_string()
}

fn create_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mdview");
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);

    for attempt in 0..1024u32 {
        let candidate = parent.join(format!(".{stem}.mdview-{pid}-{nanos}-{attempt}.tmp"));
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!(".{stem}.mdview-{pid}.tmp"))
}

#[cfg(windows)]
fn replace_file(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    use std::io;
    use std::iter;
    use std::os::windows::ffi::OsStrExt;

    type Bool = i32;
    type Dword = u32;
    type Lpcwstr = *const u16;
    const MOVEFILE_REPLACE_EXISTING: Dword = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: Dword = 0x0000_0008;

    unsafe extern "system" {
        fn MoveFileExW(
            lp_existing_file_name: Lpcwstr,
            lp_new_file_name: Lpcwstr,
            dw_flags: Dword,
        ) -> Bool;
    }

    let destination_wide: Vec<u16> = destination_path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let temp_wide: Vec<u16> = temp_path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();

    let replaced = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };

    if replaced == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(not(windows))]
fn replace_file(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, destination_path)
}

#[cfg(windows)]
fn open_shared_read(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_SHARE_DELETE: u32 = 0x0000_0004;

    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)
}

#[cfg(not(windows))]
fn open_shared_read(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::io::ErrorKind;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        detect_launch_path_from_args, replace_file, resolve_local_link_target,
        write_markdown_file_with,
    };

    #[test]
    fn extracts_first_non_flag_argument() {
        let args = vec![
            OsString::from("mdview.exe"),
            OsString::from("--dev"),
            OsString::from("C:\\notes\\todo.md"),
            OsString::from("--ignored"),
        ];

        let path = detect_launch_path_from_args(args);
        assert_eq!(path, Some(PathBuf::from("C:\\notes\\todo.md")));
    }

    #[test]
    fn atomic_write_replaces_file_contents() {
        let path = unique_test_path("atomic-write");
        fs::write(&path, "before").expect("seed markdown");

        write_markdown_file_with(&path, "after", replace_file)
            .expect("atomic write succeeds");

        let content = fs::read_to_string(&path).expect("read markdown");
        assert_eq!(content, "after");
        let temp_files = sibling_temp_files(&path);
        assert!(temp_files.is_empty(), "temporary files should be cleaned up");
        fs::remove_file(&path).expect("cleanup markdown");
    }

    #[test]
    fn atomic_write_keeps_original_when_replace_fails() {
        let path = unique_test_path("atomic-write-failure");
        fs::write(&path, "before").expect("seed markdown");

        let result = write_markdown_file_with(&path, "after", |_temp, _dest| {
            Err(std::io::Error::other("replace failed"))
        });

        assert!(result.is_err());
        let content = fs::read_to_string(&path).expect("read markdown");
        assert_eq!(content, "before");
        let temp_files = sibling_temp_files(&path);
        assert!(temp_files.is_empty(), "temporary files should be cleaned up");
        fs::remove_file(&path).expect("cleanup markdown");
    }

    #[test]
    fn atomic_write_retries_permission_denied_replace() {
        let path = unique_test_path("atomic-write-retry");
        fs::write(&path, "before").expect("seed markdown");
        let mut attempts = 0;

        write_markdown_file_with(&path, "after", |temp, dest| {
            attempts += 1;
            if attempts < 3 {
                return Err(std::io::Error::from(ErrorKind::PermissionDenied));
            }

            fs::rename(temp, dest)
        })
        .expect("retry eventually succeeds");

        let content = fs::read_to_string(&path).expect("read markdown");
        assert_eq!(content, "after");
        assert_eq!(attempts, 3);
        fs::remove_file(&path).expect("cleanup markdown");
    }

    #[test]
    fn atomic_write_reports_locked_destination_clearly() {
        let path = unique_test_path("atomic-write-locked");
        fs::write(&path, "before").expect("seed markdown");

        let error = write_markdown_file_with(&path, "after", |_temp, _dest| {
            Err(std::io::Error::from(ErrorKind::PermissionDenied))
        })
        .expect_err("locked replace should fail");

        assert!(error.contains("locked by another app"));
        fs::remove_file(&path).expect("cleanup markdown");
    }

    #[test]
    fn resolves_file_url_targets() {
        let path = std::env::temp_dir().join("mdview local link target.md");
        fs::write(&path, "test").expect("seed local link target");
        let href = format!("file:///{}", path.to_string_lossy().replace('\\', "/"));

        let resolved =
            resolve_local_link_target(None, &href).expect("file url should resolve");

        assert_eq!(resolved, path);
        fs::remove_file(&resolved).expect("cleanup local link target");
    }

    #[test]
    fn decodes_percent_escaped_file_url_targets() {
        let path = std::env::temp_dir().join("mdview local escaped target.md");
        fs::write(&path, "test").expect("seed escaped local link target");
        let href = format!(
            "file:///{}",
            path.to_string_lossy().replace('\\', "/").replace(' ', "%20")
        );

        let resolved = resolve_local_link_target(None, &href)
            .expect("percent-escaped file url should resolve");

        assert_eq!(resolved, path);
        fs::remove_file(&resolved).expect("cleanup escaped local link target");
    }

    #[test]
    fn resolves_relative_targets_against_launch_file() {
        let launch_dir = unique_test_dir("local-link-relative");
        fs::create_dir_all(launch_dir.join("nested")).expect("create relative test dir");
        let launch_path = launch_dir.join("nested").join("current.md");
        let sibling_path = launch_dir.join("other.md");
        fs::write(&launch_path, "launch").expect("seed launch file");
        fs::write(&sibling_path, "target").expect("seed target file");

        let resolved = resolve_local_link_target(Some(&launch_path), "../other.md")
            .expect("relative target should resolve");

        assert_eq!(
            fs::canonicalize(resolved).expect("canonicalize resolved path"),
            fs::canonicalize(sibling_path).expect("canonicalize sibling path")
        );
        fs::remove_dir_all(&launch_dir).expect("cleanup relative test dir");
    }

    #[test]
    fn rejects_relative_targets_without_launch_file() {
        let error = resolve_local_link_target(None, "./other.md")
            .expect_err("relative link without launch file should fail");
        assert!(error.contains("cannot resolve relative local link"));
    }

    fn unique_test_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mdview-{prefix}-{nanos}.md"))
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mdview-{prefix}-{nanos}"))
    }

    fn sibling_temp_files(path: &PathBuf) -> Vec<PathBuf> {
        let parent = path.parent().expect("temp file parent");
        let stem = path
            .file_name()
            .and_then(|value| value.to_str())
            .expect("temp file name");
        fs::read_dir(parent)
            .expect("list temp dir")
            .filter_map(|entry| entry.ok().map(|item| item.path()))
            .filter(|candidate| {
                candidate != path
                    && candidate
                        .file_name()
                        .and_then(|value| value.to_str())
                        .map(|value| value.starts_with(&format!(".{stem}.mdview-")))
                        .unwrap_or(false)
            })
            .collect()
    }
}
