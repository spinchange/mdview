# Installer Runtime Runbook

Use this runbook after significant Windows integration changes, especially:
- Preview handler lifecycle changes
- WebView2 Explorer preview rollout
- Registration/unregistration changes
- Installer script changes

## Goal
Verify that the packaged NSIS installer correctly deploys, registers, launches, previews, and cleans up `mdview` in a normal Windows user session.

## Preconditions
1. Build the shipping artifacts:
   - `viewer-shell.exe`
   - `win_preview_handler.dll`
2. Package the NSIS installer:
   - `npm run package:nsis`
3. Close running `viewer-shell.exe` / `prevhost.exe` instances if needed before retesting.

## Artifact Expectations
Installer:
- `dist\nsis\mdview-<version>-setup.exe`

Installed files:
- `%LOCALAPPDATA%\Programs\mdview\viewer-shell.exe`
- `%LOCALAPPDATA%\Programs\mdview\win_preview_handler.dll`
- `%LOCALAPPDATA%\Programs\mdview\Uninstall.exe`

## Install Smoke

### 1. Silent Install
Run:

```powershell
Start-Process "C:\Users\user\mdview\dist\nsis\mdview-0.1.0-beta.2-setup.exe" -ArgumentList "/S" -Wait
```

Confirm:
- installer exits without visible failure
- installed files exist under `%LOCALAPPDATA%\Programs\mdview`
- Start Menu shortcut exists
- desktop shortcut exists if still expected for this release

### 2. Registration Check
Run:

```powershell
& "$env:LOCALAPPDATA\Programs\mdview\viewer-shell.exe" --register
```

Confirm:
- process exits successfully
- `.md` / `.markdown` open in installed `mdview`
- Preview Pane points at installed `win_preview_handler.dll`
- icon/default-app presence looks correct

### 3. File Open Path
1. Double-click a `.md` file.
2. Confirm:
   - installed app launches
   - no localhost/dev-url error appears
   - opened file path is correct

### 4. Preview Pane Path
1. Open Explorer Preview Pane on a markdown file.
2. Confirm:
   - preview renders from installed binaries
   - Explorer does not hang
   - file switching still works

### 5. Post-Install Runtime Checks
1. Switch Windows light/dark mode if theme sync matters for the current build.
2. Confirm the app still launches after sign-out/restart if feasible.
3. If WebView2 preview is part of the current release, run:
   - `docs/POST_WEBVIEW2_VALIDATION.md`

## Uninstall Smoke

### 6. Silent Uninstall
Run:

```powershell
Start-Process "$env:LOCALAPPDATA\Programs\mdview\Uninstall.exe" -ArgumentList "/S" -Wait
```

Confirm:
- installed app directory is removed
- Start Menu shortcut is removed
- desktop shortcut is removed

### 7. Unregister Behavior
If the installer did not fully clean up integration, run:

```powershell
& "$env:LOCALAPPDATA\Programs\mdview\viewer-shell.exe" --unregister
```

Confirm:
- command exits successfully
- Preview Pane registration is removed or restored as intended
- file association/context menu state is not left broken

## Failure Signals To Watch
- installer succeeds but Preview Pane still points at a repo-local binary
- `viewer-shell.exe --register` exits non-zero
- uninstall removes files but leaves registration behind
- Preview Pane works before install but fails after install
- file-open path regresses to localhost/dev build behavior

## Likely Inspection Points If It Fails
- `scripts/nsis/mdview-installer.nsi`
  - `ExecWait '"$INSTDIR\viewer-shell.exe" "--register"'`
  - `ExecWait '"$INSTDIR\viewer-shell.exe" "--unregister"'`
- `apps/viewer-shell/src-tauri/src/main.rs`
  - CLI exit-code path for `--register` / `--unregister`
- `crates/win-installer/src/lib.rs`
  - registry write/remove logic
- installed directory under `%LOCALAPPDATA%\Programs\mdview`
- Preview Pane log:
  - `%LOCALAPPDATA%\Temp\Low\mdview-preview.log`
  - `%LOCALAPPDATA%\Temp\mdview-preview.log`

## Release Gate
Treat installer/runtime behavior as acceptable only if:
- install deploys the correct binaries
- `--register` succeeds from the installed location
- normal file-open uses installed `mdview`
- Preview Pane uses the installed handler
- uninstall and/or `--unregister` leaves the system in a sane state
