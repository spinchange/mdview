# Rendering Pipeline

## Pipeline Stages
1. Read source text from filesystem with shared access flags.
2. Normalize parse input while preserving original newline style for write-back.
3. Parse markdown with GFM support.
4. Produce:
   - render tree for viewer/preview,
   - heading/source span metadata for line mapping.
5. Apply sanitization and safe link policy.
6. Render with shared CSS token set from `base-styles`.

## Parity Policy
- `md-engine` and `base-styles` are shared dependencies for:
  - main app viewer,
  - preview handler renderer.
- Any rendering-rule changes require fixture updates and parity checks.

## Test Fixture Focus
- Large tables and multi-line table cells.
- Fenced code blocks with long lines.
- Indented code blocks.
- Mixed `\r\n` and `\n`.
- Nested blockquotes and task lists.

## Explorer WebView2 Preview Strategy

The Explorer preview handler (`win-preview-handler`) should use a WebView2 instance to render Markdown once the WebView2 upgrade lands. It should maintain close visual parity with the main app while operating under strict interaction and performance constraints.

### 1. Rendering Parity
- **Visual Goal**: The preview should look close to the main app's viewer area without reproducing full application chrome.
- **Shared Tokens**: Use `base-styles` (`ThemeTokens`) to inject CSS variables for background, text, accent, and surface colors.
- **Markdown Styles**: Ensure consistent typography (Segoe UI/Cascadia Code), line heights, and element spacing (margins/padding).
- **No Chrome**: Omit all application UI (sidebars, toolbars, buttons). The WebView2 should only contain the rendered content.

### 2. Minimum HTML Contract
The host should generate a minimal shell to wrap the `md-engine` output:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    /* 1. Injected CSS variables from ThemeTokens::to_css_vars() */
    /* 2. Base Markdown styles matching full app */
    body { background-color: var(--mdv-bg); color: var(--mdv-text); margin: 0; padding: 2rem; }
    article { max-width: 800px; margin: 0 auto; }
    /* ... additional parity styles ... */
  </style>
</head>
<body class="preview-mode">
  <article class="markdown-body">
    <!-- Rendered HTML from md-engine -->
  </article>
</body>
</html>
```

### 3. Interaction Constraints
- **Read-Only**: The preview is non-interactive.
- **Links**: 
  - External links (`https://...`) should be inert (default) or open in the user's default browser.
  - Internal links (`#heading`) should be ignored to avoid complex scroll-sync logic in the preview pane.
- **Selection**: Text selection and copying should be allowed, but no drag-and-drop or context-menu navigation.

### 4. Performance & Safety
- **Large Files**: Avoid hangs on massive files. If a file exceeds 2MB, consider rendering only the first 512KB for the preview.
- **Sanitization**: Rely on `md-engine`'s built-in Comrak sanitization (GFM tag filtering and unsafe HTML omission).
- **Statelessness**: The preview handler is a COM component hosted by `prevhost.exe`. Avoid heavy local storage usage or complex persistent state.

### 5. Validation Recommendations
- **Checklist**: Verify against `docs/PREVIEW_REGRESSION_CHECKLIST.md`.
- **Theme Sync**: Confirm the preview updates correctly when the Windows system theme (Dark/Light) or accent color changes.
- **Layout**: Ensure tables and code blocks scroll horizontally (`overflow-x: auto`) and do not break the container.
