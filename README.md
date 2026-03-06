# mdview

Windows-first, open source Markdown viewer with a deliberate native feel.

## Product Position
- Viewer first in Phase 1.
- Feature-limited editor in Phase 2 (`Quick Edit`, not an IDE).
- Tauri shell with Windows-specific integrations as core requirements.

## Non-Negotiable UX Targets
- Double-clicking a `.md` file opens quickly with no white flash.
- App follows Windows theme + accent updates automatically.
- File Explorer Preview Pane shows markdown without launching the full app.
- Viewer never blocks external editors with exclusive file locks.

## Repository Layout
- `apps/viewer-shell`: Tauri shell and frontend.
- `crates/md-engine`: markdown parse/render pipeline rules shared by app + preview.
- `crates/base-styles`: shared theme token generation and base CSS.
- `crates/win-preview-handler`: COM preview handler for Explorer Preview Pane.
- `crates/line-index`: heading/source line mapping for Viewer -> Editor jump.
- `docs`: architecture, windows integration, ADRs.

## Phase Plan
1. Native shell foundation (`no-flash` startup, file association, core viewer).
2. Windows-native polish (theme/accent sync, file watching, TOC/search/export).
3. Preview handler milestone (COM DLL + installer registration).
4. Limited editor (`Quick Edit`, Save/Undo/Redo/Find/Replace, Jump to Line).

## Status
- Scaffold created.
- Architecture docs and ADRs drafted.
- Next: implement workspace crates and boot Tauri app shell.
