# Changelog

All notable changes to this project are documented in this file.

## [0.1.0-beta.2] - 2026-03-14

### Added
- Explorer Preview Pane WebView2 rendering with markdown HTML from `md-engine`.
- Viewer external-link handling through the Tauri shell path.
- Playwright regression coverage for viewer link behavior, including:
  - external `https://` links,
  - `mailto:` links,
  - internal heading/TOC links,
  - link behavior in Quick Edit mode.
- Preview regression and release-validation docs:
  - `docs/PREVIEW_REGRESSION_CHECKLIST.md`
  - `docs/POST_WEBVIEW2_VALIDATION.md`
  - `docs/INSTALLER_RUNTIME_RUNBOOK.md`
- Preview log validation helper:
  - `tests/e2e/preview-handler.spec.ps1`

### Changed
- Explorer Preview Pane upgraded from plain-text fallback to rendered WebView2 markdown preview.
- Preview fallback/polish improved with readable typography, spacing, and explicit large-file handling before the WebView2 cutover.
- Session/release handoff docs consolidated around the current Windows integration state.

### Fixed
- Explorer preview deadlock caused by cross-process `WM_SYNCPAINT` interaction.
- Blank preview when switching files with Preview Pane already open and `SetRect` not called again.
- Viewer external links now open correctly without breaking internal `#heading` navigation or Quick Edit heading jumps.

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
- Explorer preview handler scaffold upgraded into a stable plain-text fallback host.
- Preview rendering now supports relative markdown assets via virtual host mapping.
- Preview fallback behavior improved: themed error HTML instead of hard handler failure.
- Theme watcher now reads registry in-process (no `reg.exe` subprocess polling), reducing flicker/popups.
- Tauri build output stabilized for embedded frontend assets from `web/dist`.
- Application icon updated to terminal-inspired aesthetic.

### Fixed
- Resolved duplicate-window creation crash by relying on Rust window creation with `tauri.conf.json` windows set to empty.
- Corrected capability mapping to valid Tauri v2 permissions for `"main"` window.
- Improved unload/resource cleanup paths in preview handler.
