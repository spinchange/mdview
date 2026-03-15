# Post-WebView2 Validation

Use this checklist after the Explorer preview pane is upgraded from the EDIT-based raw-text preview to a WebView2 rendered markdown preview.

## Goal
Confirm that the rendered Explorer preview is an upgrade in product quality without reintroducing lifecycle, registration, or install/runtime regressions.

## Preconditions
1. Build the updated preview handler and deploy it beside the installed `viewer-shell.exe`.
2. Re-register shell integration with the current installed binary.
3. Clear the preview log:
   - `tests/e2e/preview-handler.spec.ps1 -ClearLog`
4. Prepare these sample files:
   - normal markdown file
   - file with headings + internal `#heading` links
   - file with external `https://` links
   - file with code fences + table
   - malformed markdown file
   - empty markdown file
   - large markdown file

## Functional Checks

### 1. First Open
1. Open Explorer with Preview Pane disabled.
2. Select a `.md` file.
3. Enable Preview Pane.
4. Confirm:
   - preview appears
   - Explorer does not hang
   - rendered content is visible, not a blank/white WebView

### 2. File Switching
1. Leave Preview Pane open.
2. Click through multiple markdown files.
3. Confirm:
   - every file renders
   - no stale content remains from previous file
   - no blank pane on file switch

### 3. Unload/Reopen
1. Close Preview Pane while a markdown preview is visible.
2. Reopen the pane.
3. Switch files again.
4. Confirm:
   - preview returns cleanly
   - no hang or white surface after reopen

### 4. Resize
1. Resize Explorer horizontally and vertically with Preview Pane open.
2. Confirm:
   - rendered preview resizes with the pane
   - no clipped or frozen region remains
   - no severe layout breakage at narrow widths

## Rendering Quality Checks

### 5. Markdown Fidelity
Use a file with headings, paragraphs, code fences, and tables.

Confirm:
- headings are visually distinct
- body text is readable
- code blocks are legible
- tables are readable enough for preview use
- no obvious raw-HTML leakage or unsafe rendering

### 6. Link Policy
Use a file with external `https://` links and internal `#heading` links.

Confirm:
- behavior matches the intended Explorer preview policy
- links do not hang Explorer
- if links are intentionally inert in Explorer preview, they fail inertly and consistently

### 7. Large File Behavior
Use a markdown file that exceeds the intended preview threshold.

Confirm:
- preview does not hang
- truncation/fallback behavior is explicit and readable if still applicable
- Explorer remains responsive after selecting the file

## Install/Runtime Checks

### 8. Installed Binary Path
1. Run the packaged installer in a normal Windows session.
2. Confirm installed files exist in `%LOCALAPPDATA%\Programs\mdview`.
3. Confirm Preview Pane uses the installed `win_preview_handler.dll`, not a repo-local binary.

### 9. File Open Path
1. Double-click `.md` and `.markdown` files after install.
2. Confirm:
   - installed `mdview` launches
   - no localhost/dev-url launch errors
   - correct app icon remains in Explorer

### 10. Uninstall
1. Run uninstall.
2. Confirm:
   - installed app directory is removed
   - Preview Pane registration is removed or restored to the intended prior state
   - file-open/default-app behavior is not left in a broken state

## Log Validation
Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "C:\Users\user\mdview\tests\e2e\preview-handler.spec.ps1" -RequireShow
```

Confirm:
- `SetWindow`, `DoPreview`, and at least one valid show path are present
- no obvious lifecycle regression compared with the EDIT-based version
- no repeated failure pattern that leaves Explorer hung or preview blank

## Release Gate
Treat the WebView2 preview as ready only if all of the following are true:
- no Explorer hangs
- no blank preview on first open or file switch
- rendered markdown is materially better than the EDIT-based fallback
- install/re-register/uninstall paths still behave correctly
- normal file open in `mdview` remains intact
