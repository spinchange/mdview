# Architecture

## Goals
- Deliver a Windows-native markdown viewer experience, not a generic web wrapper.
- Isolate high-risk Windows components (Preview Handler, registration, startup).
- Keep rendering parity between main app and Explorer Preview Pane.

## System Components

### App Shell (`apps/viewer-shell/src-tauri`)
- Owns window lifecycle, startup sequencing, OS integration bridges.
- Creates hidden window and shows only after first paint readiness.
- Resolves startup theme/accent and injects initial background before show.

### Frontend (`apps/viewer-shell/web`)
- Owns view composition and UI state.
- Viewer is default mode.
- Editor is feature-limited and only active in `Quick Edit`.

### Shared Rendering Core (`crates/md-engine`)
- Defines markdown parsing and transformation rules.
- Must stay lightweight and portable for both app and COM host usage.
- No dependency on UI runtime state or Tauri internals.

### Shared Styles (`crates/base-styles`)
- Canonical theme token model and generated base CSS.
- Consumed by:
  - Main app frontend (full UI).
  - Preview handler host renderer (preview-only surface).

### Preview Handler (`crates/win-preview-handler`)
- Separate COM component loaded by `prevhost.exe`.
- Stateless by design: receives file context, renders preview, exits.
- Cannot assume app process, app settings store, or long-lived session state.

### Line Mapping (`crates/line-index`)
- Provides shared source line/offset primitives for LF, CRLF, and mixed-newline documents.
- Exposed to the shell through `line_map_bridge::line_to_offset` for future editor jump-offset commands.
- Current heading jumps use Comrak source positions from `md-engine` and CodeMirror's document line API in the frontend.
- Intended next integration point: route all Viewer heading -> Editor offset conversion through `line-index` so the documented bridge has one canonical mapping path.

## State Boundary (Explicit Ownership)

### Rust-owned State
- File system path resolution and safe local IO policies.
- OS integration state:
  - File associations.
  - Theme/accent watcher events.
  - Preview handler registration/runtime wiring.
- Startup orchestration state:
  - First paint gating.
  - Initial theme seed values.
- Durable settings storage API shape and persistence guarantees.
- Heading/source metadata produced by `md-engine`, with source offset helpers supplied by `line-index`.

### Frontend-owned State
- View presentation state:
  - TOC open/closed, active heading, scroll position.
  - Search panel visibility and local query terms.
- Viewer ephemeral UI state:
  - Current zoom/font scale (after load from settings).
  - Rendered document navigation state.
- Editor session state (Phase 2):
  - Undo/redo stack.
  - Dirty flag.
  - Selection and cursor position.

### Shared Contract State (Bridge Messages)
- Document model version hash.
- Theme token payload.
- Heading id -> line number map.
- File change notifications (debounced).
- Save/command acknowledgements.

## Data Flow
1. Shell receives file path from file association or open action.
2. Rust loads file with shared-read/shared-write/shared-delete flags.
3. `md-engine` returns render model and heading/source metadata.
4. Frontend renders viewer using shared styles from `base-styles`.
5. If `Quick Edit` is enabled, editor consumes same source and line map.
6. Viewer heading click dispatches a source line number from the rendered heading metadata.

## Hard Constraints
- No exclusive file locks for viewer reads.
- Debounced file-watch update pipeline (target ~100ms).
- Startup must avoid pre-theme white flash.
- Preview handler must not import app-only runtime assumptions.
- Main app rendering must avoid synchronously parsing very large files; current beta limit is 2 MB.

## Definition of Done Anchors
- First visible frame uses correct theme + vibrancy with zero white flash.
- Explorer Preview Pane renders markdown without starting full app.
- Viewer heading click lands editor cursor on the source line reported by the Markdown parser.
- External editor saves do not flicker or conflict with viewer locks.
