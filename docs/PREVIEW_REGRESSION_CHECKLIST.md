# Preview Regression Checklist

Use this checklist when changing `crates/win-preview-handler` or Explorer registration behavior.

## Target Regressions
- Deadlock or hang in `prevhost.exe` / Explorer during preview creation
- Blank preview when switching files while Preview Pane is already open
- Preview not returning after pane close/reopen
- Broken sizing after Explorer window resize
- Shell registration drift after rebuild or reinstall
- WebView2-specific blank/white preview surface after successful handler activation
- Regressions where rendered preview works but file-open/runtime registration no longer does

## Preconditions
1. Build and deploy the current preview handler and viewer-shell artifacts.
2. Register shell integration with the current binary.
3. Close existing `explorer.exe` / `prevhost.exe` instances only if needed for a clean retry.
4. Keep a few sample files ready:
   - small normal markdown file
   - markdown file with headings and internal `#anchor` links
   - markdown file with external `https://` links
   - markdown file with code fence + table
   - empty markdown file
   - malformed markdown file
   - large markdown file

## Preview Lifecycle Matrix

### Case 1: First Open With Zero Rect
1. Open Explorer with Preview Pane disabled.
2. Select a `.md` file.
3. Enable Preview Pane.
4. Expected:
   - preview appears
   - Explorer does not hang
   - no permanently blank pane

### Case 2: File Switch With Preview Pane Already Open
1. Leave Preview Pane open.
2. Click through 3-5 different `.md` files in the same folder.
3. Repeat in a second folder.
4. Expected:
   - every file renders
   - no stale content from prior file
   - no blank pane after the first successful preview

### Case 3: Unload And Reopen
1. With a preview visible, close Preview Pane.
2. Reopen Preview Pane on the same file.
3. Switch to another markdown file.
4. Expected:
   - preview returns after reopen
   - switching still works after the pane was unloaded

### Case 4: Resize Behavior
1. With a preview visible, resize Explorer narrower and wider.
2. Resize Explorer taller and shorter.
3. Expected:
   - preview resizes with the pane
   - no clipped frozen area
   - no repaint glitches severe enough to obscure content

### Case 5: Error/Edge Inputs
1. Preview an empty markdown file.
2. Preview malformed markdown.
3. Preview a large markdown file.
4. Expected:
   - no shell-level error UI
   - fallback/truncation behavior is readable and intentional
   - no hang on large input

### Case 6: Rendered Markdown Quality
1. Preview a file with headings, paragraphs, links, code fences, and a table.
2. Expected:
   - headings are visually distinct
   - paragraphs are readable at normal pane widths
   - code blocks are legible and contained
   - tables are readable enough for preview use
   - no obvious raw-HTML leakage

### Case 7: Link Behavior Policy
1. Preview a file with external `https://` links.
2. Preview a file with internal `#heading` links.
3. Expected:
   - behavior matches the intended Explorer preview policy for this release
   - links do not hang Explorer
   - if links are intentionally inert in preview, that should be consistent

## Registration Checks
1. Run `viewer-shell.exe --register`.
2. Verify `.md` and `.markdown` still preview in Explorer.
3. Restart Explorer and retest one file.
4. Reboot or sign out/in and retest one file.
5. Verify normal file-open still launches `mdview` and not a stale repo/dev binary.
6. Verify icon/default-app state did not regress after preview-handler changes.

## Logs
Look for preview handler logs in one of these locations:
- `%LOCALAPPDATA%\Temp\Low\mdview-preview.log`
- `%LOCALAPPDATA%\Temp\mdview-preview.log`

Useful signals:
- `SetWindow`
- `DoPreview`
- `SetRect`
- `thread: child=`
- `Unload`

Expected patterns:
- First-open path usually shows zero-area `SetWindow`/`DoPreview`, then real-area `SetRect`
- File-switch path may show `DoPreview` with `do_show=true` and no follow-up `SetRect`

## Pass Criteria
- No Explorer hangs
- No blank preview on file switching
- No preview loss after unload/reopen
- Predictable behavior on malformed/empty/large files
- Rendered markdown is readable enough to justify WebView2 complexity
- Registration survives rebuild/redeploy/restart cycles
