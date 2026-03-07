# mdview

Windows-first Markdown viewer with native shell integration.

## About
`mdview` is focused on fast, clean markdown reading on Windows, with Explorer integration built in:
- native-feeling startup (no flash gate),
- live theme/accent sync,
- Explorer Preview Pane rendering via COM preview handler,
- Open With / context menu / default-app registration helpers.

This project is currently in `v0.1.0-beta.1`.

## Current Capabilities
- Markdown rendering pipeline (`md-engine`, comrak-based, source position aware).
- Tauri desktop shell + frontend viewer UI.
- Explorer Preview Handler using WebView2, with resilient fallback pages.
- Relative asset support in preview via virtual host folder mapping.
- Shared config-backed theme resolution (`%APPDATA%\\mdview\\config.json`).
- Native registration commands:
  - `--register`
  - `--unregister`

## Build
From repository root:

```powershell
cargo check --workspace
```

Release app packaging:

```powershell
cd apps/viewer-shell/src-tauri
cargo tauri build
```

Primary release binary:

`target\\release\\viewer-shell.exe`

## First Run
Register shell integration:

```powershell
target\release\viewer-shell.exe --register
```

This registers:
- Preview Handler for `.md` / `.markdown`
- Context menu verb (`Open with mdview`)
- Open With + Default Apps capability metadata

After register, set defaults in Windows Settings for `.md` and `.markdown` (mdview opens this page automatically when possible).

## Repository Layout
- `apps/viewer-shell`: Tauri shell + web UI.
- `crates/md-engine`: markdown parse/render pipeline.
- `crates/base-styles`: theme token to CSS variables bridge.
- `crates/win-preview-handler`: Explorer Preview Pane COM handler.
- `crates/win-installer`: native registry registration/unregistration logic.
- `crates/settings-store`: shared app config and theme preference resolution.
- `docs`: architecture notes, ADRs, and release notes.
