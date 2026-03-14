use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

#[derive(Debug, Clone)]
pub struct LaunchPathState {
    launch_path: Option<PathBuf>,
}

impl LaunchPathState {
    pub fn new(launch_path: Option<PathBuf>) -> Self {
        Self { launch_path }
    }

    pub fn as_string(&self) -> Option<String> {
        self.launch_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
    }

    pub fn path_clone(&self) -> Option<PathBuf> {
        self.launch_path.clone()
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
    match &state.launch_path {
        Some(path) => read_markdown_file_impl(path).map(Some),
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
    F: Fn(&Path, &Path) -> std::io::Result<()>,
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
        replace(&temp_path, path)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result.map_err(|e| format!("failed to write markdown file: {e}"))
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{detect_launch_path_from_args, replace_file, write_markdown_file_with};

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

    fn unique_test_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mdview-{prefix}-{nanos}.md"))
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
