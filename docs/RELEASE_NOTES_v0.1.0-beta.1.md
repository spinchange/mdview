# mdview v0.1.0-beta.1

## Highlights
- Windows-native markdown viewer shell with no-flash startup handoff.
- Explorer Preview Handler scaffold upgraded to real WebView2 rendering.
- Relative asset loading in preview via virtual host folder mapping.
- Resilient preview fallback pages on read/render errors (no handler hard-fail).
- Shared config bridge for theme overrides across Tauri app and preview handler.
- Native Rust shell registration flow (`--register` / `--unregister`), including:
  - preview handler registration,
  - context menu integration,
  - Open With and Default Apps capability registration.
- Theme watcher stabilized to in-process registry reads (no subprocess flicker).
- Tauri production artifact pipeline validated with embedded frontend dist assets.

## Known Limitations
- Windows 11 top-level context menu placement still depends on system policy for classic verbs.
- Bundle identifier currently uses `io.mdview.app`; functional on Windows, but should be normalized before stable release.
- Minimal editor (`Quick Edit`) remains out of scope for this beta.

## First-Run
1. Build artifact: `target\\release\\viewer-shell.exe`
2. Register shell integration:
   - `viewer-shell.exe --register`
3. Optional cleanup:
   - `viewer-shell.exe --unregister`
