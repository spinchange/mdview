# Session Handoff (2026-03-06)

## Completed This Session
- Implemented no-flash startup gate with hidden window + explicit `window_ready` command + 500ms safety timeout.
- Added deterministic theme bootstrap contract:
  - frontend listens first,
  - frontend invokes `get_initial_theme_css`,
  - then calls `window_ready`.
- Implemented `win-theme-watcher` + `base-styles` token pipeline and runtime theme update event.
- Implemented `md-engine` with `comrak`:
  - GFM-related options enabled,
  - heading/source position extraction,
  - unsafe HTML disabled,
  - blank-input handling,
  - serde derives for Tauri bridge transport.
- Implemented `render_markdown` Tauri command.
- Added viewer rendering layer in frontend:
  - HTML mount,
  - TOC generation,
  - heading DOM annotation with `data-line-start`/`data-line-end`.
- Implemented file loading:
  - launch path detection from argv,
  - `read_launch_markdown` and `read_markdown_file` commands,
  - shared-read open flags on Windows.
- Implemented live reload (Phase 3.1):
  - `notify`-based watcher with ~100ms debounce,
  - emits `mdview://file-changed`,
  - frontend re-reads + re-renders on event.
- Implemented COM preview-handler plumbing:
  - `win-preview-handler` as `cdylib`,
  - `IInitializeWithFile` + `IPreviewHandler` scaffold,
  - class factory (`IClassFactory`) with proper query path,
  - aggregation rejection (`CLASS_E_NOAGGREGATION`),
  - unload accounting via active object count,
  - `DllCanUnloadNow` / `DllGetClassObject`.
- Added dev registration script:
  - `scripts/register-dev.ps1`,
  - per-user COM registration + extension binding for `.md`/`.markdown`,
  - ProgID mapping and unregister support.
- Wired shutdown cleanup:
  - stop file watcher and theme watcher on app exit events.

## Validation
- `cargo check --workspace` passes on Windows.

## Known Scope Left
- Phase 1.5 final boss: embed actual WebView2 rendering host in `win-preview-handler` `DoPreview`.
- Phase 3 image asset protocol bridge for local relative images in markdown.
- Phase 2.1 editor: first CodeMirror toggle and basic edit command path.

## Practical Next Step
1. Implement preview handler rendering host (WebView2 child inside `parent_hwnd`).
2. Feed it `md-engine` HTML + `base-styles` CSS tokens for parity with main app.
