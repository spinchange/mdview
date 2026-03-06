# Backlog

## Initial Issues
1. ADR confirmation: Tauri shell + separate preview handler architecture.
2. Implement hidden window startup with first-paint show gate.
3. Implement pre-show theme seed and no-white-flash boot skeleton.
4. Add `.md` / `.markdown` file association (per-user default, all-users option).
5. Build shared `md-engine` crate with baseline GFM fixture tests.
6. Build shared `base-styles` crate and token export format.
7. Implement `win-theme-watcher` crate and frontend token bridge.
8. Implement `line-index` crate using rope-based line mapping.
9. Add viewer TOC extraction and heading navigation.
10. Add `Quick Edit` toggle and basic CodeMirror host.
11. Add Save/Undo/Redo/Find/Replace commands.
12. Implement Viewer->Editor jump-to-line bridge.
13. Scaffold `win-preview-handler` COM DLL host + registration path.
14. Add file watcher debounce/coalescing and atomic-save handling tests.
15. Add startup, theme-sync, preview, and jump bridge e2e tests.
