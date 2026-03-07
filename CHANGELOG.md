# Changelog

All notable changes to this project are documented in this file.

## [0.1.0-beta.1] - 2026-03-07

### Added
- Native Windows shell registration in Rust (`win-installer` crate):
  - preview handler registration,
  - context menu registration,
  - Open With + Default Apps capabilities registration,
  - unified unregister cleanup.
- CLI integration for shell ops:
  - `viewer-shell.exe --register`
  - `viewer-shell.exe --unregister`
- Shared config resolution (`settings-store`) with `%APPDATA%\\mdview\\config.json` support for theme preferences.
- In-app “Set as default” helper action to open Windows Default Apps settings.
- Release notes doc: `docs/RELEASE_NOTES_v0.1.0-beta.1.md`.

### Changed
- Explorer preview handler upgraded from scaffold to WebView2-based rendering host.
- Preview rendering now supports relative markdown assets via virtual host mapping.
- Preview fallback behavior improved: themed error HTML instead of hard handler failure.
- Theme watcher now reads registry in-process (no `reg.exe` subprocess polling), reducing flicker/popups.
- Tauri build output stabilized for embedded frontend assets from `web/dist`.
- Application icon updated to terminal-inspired aesthetic.

### Fixed
- Resolved duplicate-window creation crash by relying on Rust window creation with `tauri.conf.json` windows set to empty.
- Corrected capability mapping to valid Tauri v2 permissions for `"main"` window.
- Improved unload/resource cleanup paths in preview handler.
