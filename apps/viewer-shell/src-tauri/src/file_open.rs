use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

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
    use std::path::PathBuf;

    use super::detect_launch_path_from_args;

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
}
