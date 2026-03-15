# mdview Beta Test Checklist

Use this checklist during daily beta testing to capture actionable issues quickly.

## Daily Smoke
1. Launch app directly from Start/Search.
2. Launch by double-clicking `.md`.
3. Launch from right-click `Open with mdview`.
4. Open 3 files in different folders, including one large file.
5. Confirm no flicker/window pop behavior.

## Default App
1. Set mdview as default for `.md`.
2. Set mdview as default for `.markdown`.
3. Verify both extensions open in mdview after reboot/sign-out.
4. Verify mdview appears in Default Apps/Open With consistently.

## Preview Pane
1. Enable Explorer Preview Pane and click `.md` files.
2. First-open lifecycle: with Preview Pane initially closed, select a `.md` file, then open Preview Pane and confirm content appears.
3. File-switch lifecycle: leave Preview Pane open and click through 3-5 different `.md` files; confirm every file renders and no blank pane appears.
4. Unload/reopen lifecycle: close Preview Pane, reopen it, then switch files again; confirm preview still appears.
5. Resize Explorer horizontally and vertically with Preview Pane open; confirm preview resizes and remains readable.
2. Test file with relative image links (`./img.png`).
3. Test malformed markdown + empty file + missing file target.
4. Confirm fallback/error page is readable (not generic shell failure).
5. Re-test after `--register`, Explorer restart, and reboot.
6. If troubleshooting, capture `%LOCALAPPDATA%\Temp\Low\mdview-preview.log` or `%LOCALAPPDATA%\Temp\mdview-preview.log`.

## Live Reload + External Edit
1. Open file in mdview and edit in VS Code/Notepad.
2. Confirm update appears without locking/saving issues.
3. Rapid-save 5-10 times; check no stale state or crashes.

## Viewer Links
1. Open a markdown file in mdview containing external links and `#heading` links.
2. Click an external `https://` link; confirm it opens in the default browser.
3. Click an internal `#heading` link; confirm it navigates within the document instead of leaving the app.
4. Enable Quick Edit and verify heading click-to-line still works after link handling changes.

## Theme / Appearance
1. Switch Windows light/dark mode while app is open.
2. Change accent color.
3. Confirm app + preview pane update appropriately.
4. Verify icon appearance in taskbar/start/file associations.

## Registration / Unregistration
1. Run `--unregister`; verify integration removed.
2. Run `--register`; verify integration restored.
3. Move/copy exe location and re-run `--register`; verify paths update.

## Performance
1. Cold start timing (feel + rough seconds).
2. Open large markdown file timing.
3. Memory feel after prolonged use (no obvious runaway).

## Bug Report Template
1. Title
2. Steps to reproduce
3. Expected vs actual
4. Frequency (always/intermittent)
5. Environment (Windows build, file type, path)
6. Attach sample file if relevant
