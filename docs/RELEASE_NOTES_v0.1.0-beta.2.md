# mdview v0.1.0-beta.2

## Highlights
- Explorer Preview Pane now renders markdown through WebView2 instead of the plain-text fallback.
- Preview lifecycle issues in Explorer were fixed:
  - no more cross-process preview deadlock,
  - no more blank preview when switching files with the pane already open.
- Viewer external links now open correctly through the Tauri shell path.
- Viewer link regressions are covered for:
  - `https://` links,
  - `mailto:` links,
  - internal heading and TOC links,
  - Quick Edit interactions.
- Validation/runbook coverage was expanded for:
  - preview lifecycle regression checks,
  - post-WebView2 validation,
  - installer/runtime smoke testing.

## Known Limitations
- Explorer preview resize is functional but can still feel visually jerky during pane drag.
- Explorer preview link policy is not fully finalized; some markdown links may remain inert depending on rendered output.
- Installed-build smoke should still be run before treating this as a wider release candidate.
- Bundle identifier still uses `io.mdview.app`; functional on Windows, but should be normalized before stable release.

## First-Run
1. Build artifact: `target\\release\\viewer-shell.exe`
2. Register shell integration:
   - `viewer-shell.exe --register`
3. Optional cleanup:
   - `viewer-shell.exe --unregister`

## Validation Docs
- `docs/PREVIEW_REGRESSION_CHECKLIST.md`
- `docs/POST_WEBVIEW2_VALIDATION.md`
- `docs/INSTALLER_RUNTIME_RUNBOOK.md`
