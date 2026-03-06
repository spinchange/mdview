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
