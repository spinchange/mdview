# Editor Scope

## Positioning
Editor is a convenience mode for small edits, not a full IDE.

## Included (Phase 2)
- `Quick Edit` toggle and hotkey.
- Save (`Ctrl+S`).
- Undo/Redo.
- Find/Replace (single-file).
- Viewer heading click -> editor jump-to-line.

## Excluded
- LSP, diagnostics, code actions.
- Multi-file project editing.
- Terminal integration.
- Refactoring tools.
- Extension marketplace/plugins.

## Constraints
- Editor remains optional and hidden by default.
- Viewer experience must not regress because of editor state complexity.
- All jump-to-line positions sourced from canonical `line-index` mappings.
