# Windows Integration

## Scope
Windows-specific behavior is a first-class requirement:
- File association open path.
- Theme/accent synchronization.
- Explorer Preview Pane support.
- Native-feeling startup and window treatment.

## Startup: No Flash Strategy
- Create Tauri window hidden.
- Resolve system theme before showing window.
- Inject initial shell background that matches resolved theme.
- Apply Mica/Acrylic treatment before first visible frame.
- Show window on first paint-ready signal with timeout fallback.

## File Association
- Register `.md` and `.markdown` for per-user install by default.
- `--all-users` installer mode is planned but not implemented yet.
- On open, pass full file path to shell and load immediately.
- Shell captures launch args and exposes startup file contents via `read_launch_markdown`.
- Rust watcher emits `mdview://file-changed` after debounced filesystem updates (~100ms) for live reload.

## Theme + Accent Sync
- `win-theme-watcher` publishes:
  - dark/light mode changes,
  - accent color changes.
- Frontend consumes normalized token payload and updates CSS variables.
- Shared token model lives in `base-styles` to keep parity with preview host.
- Tauri shell emits `mdview://theme-updated` with CSS variable payload whenever tokens change.
- Frontend should register the event listener first, then invoke `get_initial_theme_css` for deterministic first-theme application.

## Explorer Preview Handler
- Implement as separate COM DLL (`win-preview-handler`).
- Host process is `prevhost.exe`; component must remain stateless.
- Share parser and theme token outputs with main app crates.
- COM registration handled in code via `windows-rs`.
- Current install assumption: `win_preview_handler.dll` is deployed beside `viewer-shell.exe`.
- Dev registration helper: `scripts/register-dev.ps1` (supports `-Unregister`).
- Explorer may require a restart, or the user may need to sign out and back in, before handler registration changes appear in Preview Pane.

## WebView2 Runtime Policy
- Use Evergreen runtime.
- Installer bootstrap check:
  - detect missing/too-old runtime,
  - prompt install/fetch when needed.
- Do not bundle fixed runtime by default due package-size impact.

## Filesystem Hygiene
- Open file handles with read/write/delete sharing flags.
- Coalesce file watcher bursts with debounce (~100ms target).
- Handle atomic-save patterns (temp write + rename/replace).

## Validation Matrix
- Win11 dark/light scheduled switch.
- Win11 accent change while app running.
- Explorer Preview Pane rendering for local markdown and images.
- Double-click open cold start and warm start.
