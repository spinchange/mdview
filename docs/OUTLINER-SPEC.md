# Outliner App Spec

A standalone, serverless outliner editor with Org-mode conventions, built on
markdown files with YAML frontmatter. Forked from Notepad.

---

## 1. File Format

Files are plain text with a `.md` extension. YAML frontmatter delimited by
`---` is optional. Heading syntax uses `#` (not Org's `*`), meaning files are
valid, portable markdown that renders correctly in GitHub, VS Code, and any
markdown-aware tool.

**Decision: `.md` not `.org`.**
Consequence: files are not directly openable in Emacs as Org files. If Emacs
compatibility becomes a requirement later, the heading syntax would need to
change. Accepted tradeoff for now.

---

## 2. Frontmatter Schema

All keys are optional. A file with no frontmatter is a plain document.

```yaml
---
outline: true          # enables outline mode (see §3)
startup: overview      # initial fold state on open (see §4.1)
todo: [TODO, NEXT, WAITING]        # active-state keywords
done: [DONE, CANCELLED]            # done-state keywords
tags: [work, personal, someday]    # valid tags (for autocomplete)
toc: true              # show TOC in read mode (inherited from Notepad)
---
```

### 2.1 `outline`

| Value | Meaning |
|---|---|
| absent / `false` | Document mode. No outliner features active. |
| `true` | Outline mode, open fully expanded (showall). |
| `"overview"` | Outline mode, open with only top-level headings visible. |
| `"contents"` | Outline mode, open with all headings visible, body text folded. |

`startup` is an alias for `outline` when the value is `overview` or `contents`.
If both are present, `outline` wins.

### 2.2 `todo` / `done`

Defines the TODO keyword sequences for this file. Keywords in `todo` are
active states; keywords in `done` are terminal states.

If absent, the app falls back to a global default (see §5.1).

### 2.3 `tags`

A list of known tags for this file. Tags not in this list are still valid —
they just don't get autocomplete. Tag validation is never enforced.

---

## 3. Editor Modes

### 3.1 Document Mode (default)

The editor behaves as it does in Notepad today. No outliner UI, no special
heading behavior, no keybinding overrides. `TAB` indents normally.

### 3.2 Outline Mode

Activated when `outline` is `true`, `"overview"`, or `"contents"` in
frontmatter. In this mode:

- Headings (`#`–`######`) are outline nodes.
- A **section** is a heading plus all content up to the next heading of equal
  or higher level. This is the atomic unit for fold, move, and promote.
- Body text (non-heading content) within a section travels with its heading.
- Content before the first heading is **preamble** — preserved, never folded,
  not part of any section.
- Fold indicators appear in the gutter on heading lines.

The mode does not prevent writing prose. A heading with a body of several
paragraphs is valid and normal.

---

## 4. Outliner Behavior

### 4.1 Fold States

Each heading cycles through three states:

| State | What is visible |
|---|---|
| `FOLDED` | Heading line only. Body text and all child headings hidden. |
| `CHILDREN` | Heading + direct child headings (their content folded). |
| `SUBTREE` | Everything under this heading, fully expanded. |

**Global fold states (S-TAB):**

| State | What is visible |
|---|---|
| `OVERVIEW` | Top-level headings only. |
| `CONTENTS` | All headings, body text folded. |
| `SHOWALL` | Everything expanded. |

Global state cycles: OVERVIEW → CONTENTS → SHOWALL → OVERVIEW.

### 4.2 Initial Fold State

Determined by the `outline` frontmatter value on open:

- `true` → SHOWALL
- `"overview"` → OVERVIEW
- `"contents"` → CONTENTS

### 4.3 Fold State Persistence

**Decision: fold state is ephemeral.** It resets to the frontmatter default on
every open. It is not saved to the file or to localStorage.

Rationale: saving fold state out-of-band (localStorage) creates a hidden
dependency between the app and the file. The frontmatter default is the
explicit, portable, file-carried specification of intent.

### 4.4 Move Behavior (M-UP / M-DOWN)

Moves the current section (heading + all its content) up or down, swapping
with the adjacent sibling section. A sibling is a heading at the same level
with the same parent.

A section cannot be moved past a heading of a different level — only past
headings of equal level within the same parent. If no valid swap target exists,
the command is a no-op.

### 4.5 Promote / Demote

`M-LEFT` promotes the heading by one level (`##` → `#`). `M-RIGHT` demotes
by one level (`#` → `##`).

`M-S-LEFT` / `M-S-RIGHT` promote/demote the entire subtree — the heading and
all its children — adjusting all levels by the same delta.

**Edge case:** Promoting a top-level heading (`#`) is a no-op. Demoting a
heading that would push any child beyond H6 is a no-op (or clamped — TBD).

### 4.6 Insert Heading (M-RET)

- If cursor is on a heading line: inserts a new heading of the same level on
  the next line, after the current section.
- If cursor is in body text: inserts a new heading of the same level as the
  nearest ancestor heading, placed after the current section.
- If cursor is in preamble (before first heading): inserts a top-level heading.

---

## 5. TODO Keywords

### 5.1 Global Defaults

If a file's frontmatter has no `todo`/`done` keys, the app uses:

```
todo: [TODO]
done: [DONE]
```

These can be changed in app settings but are not stored in any file.

### 5.2 Syntax

A heading is a TODO heading when its text begins with a recognized keyword,
separated from any `#` markers by a single space:

```
## TODO Fix the broken thing
## DONE Write the spec
## WAITING Hear back from Chris
```

The keyword must be the first word of the heading text. Keywords are
case-sensitive and must match exactly.

### 5.3 Rendering

- Active-state keywords (from `todo`) are rendered with a distinct color
  (e.g. amber).
- Done-state keywords (from `done`) are rendered muted, with the heading text
  struck through or de-emphasized.
- Non-keyword headings render normally.

### 5.4 Cycling (C-c C-t)

With cursor on a TODO heading, cycles through the keyword sequence:

`(none)` → `TODO` → `NEXT` → `WAITING` → `DONE` → `CANCELLED` → `(none)`

The cycle follows the order: all `todo` keywords in order, then all `done`
keywords in order, then back to no keyword.

When a heading transitions to a done-state keyword, a `CLOSED:` timestamp is
inserted on the first line of the body:

```
CLOSED: [2026-05-13]
```

When it transitions away from a done state, the `CLOSED:` line is removed.

---

## 6. Tags

### 6.1 Syntax

Tags appear at the end of a heading line, separated by whitespace, using Org's
colon-delimited format:

```
## My Heading :work:urgent:
## Another Heading :personal:
```

Multiple tags are written inside a single colon-wrapped group: `:tag1:tag2:`.
Each tag is alphanumeric with hyphens allowed (`my-tag`). No spaces inside tags.

### 6.2 Rendering

In outline mode, tags are rendered as small chips/pills at the end of the
heading line, visually distinct from heading text.

### 6.3 Autocomplete

When typing `:` at the end of a heading, the editor offers autocomplete from
the file's `tags` frontmatter list. Tags not in the list can still be typed
freely.

### 6.4 Tag Inheritance

**Decision: no tag inheritance in v1.** Tags apply only to the heading they
are written on. Children do not inherit parent tags.

Rationale: inheritance requires traversing the section tree on every query and
makes the mental model more complex. Can be added later.

### 6.5 Tag Filtering

A tag filter UI (sidebar or toolbar) lets you show only sections whose heading
carries a given tag. Sections not matching are hidden. Preamble is always shown.

**Decision: filtering is within the current file only.** Cross-file tag
queries belong to the agenda (see §9).

---

## 7. Scheduling & Deadlines

### 7.1 Syntax

Scheduling timestamps appear on the first line(s) of a section's body, not on
the heading line itself:

```
## TODO Submit the report :work:
SCHEDULED: <2026-05-15>
DEADLINE: <2026-05-20>
```

Active timestamps use angle brackets `<>`. Inactive timestamps (not surfaced
in agenda) use square brackets `[]`.

### 7.2 Date Format

```
<YYYY-MM-DD>          # date only
<YYYY-MM-DD HH:MM>    # date and time
```

Repeaters (e.g. `+1w`) are **out of scope for v1**.

### 7.3 Inserting Timestamps

- `C-c C-s` → set/replace SCHEDULED timestamp on current heading (opens date picker)
- `C-c C-d` → set/replace DEADLINE timestamp on current heading (opens date picker)

Date picker is a minimal inline popup. Keyboard-navigable.

### 7.4 Rendering

In outline mode, SCHEDULED and DEADLINE lines are rendered with distinct
styling (e.g. small, colored, icon-prefixed) rather than as plain monospace
text.

---

## 8. Property Drawers

### 8.1 Syntax

A property drawer immediately follows a heading's SCHEDULED/DEADLINE lines
(or the heading itself if none), before any body prose:

```
## My Heading
:PROPERTIES:
:ID:       abc-123
:EFFORT:   2h
:CUSTOM:   any value
:END:
```

The drawer is bounded by `:PROPERTIES:` and `:END:`. Property keys are
uppercase by convention but not enforced. Values are freeform strings.

### 8.2 Rendering & Interaction

- Drawers are rendered collapsed by default in outline mode, showing only a
  `⋯ properties` indicator.
- Clicking the indicator (or pressing `TAB` with cursor on the drawer)
  toggles expanded/collapsed.
- In expanded state, properties are editable inline.

### 8.3 The `:ID:` Property

`:ID:` is reserved as a unique, stable identifier for a heading. The app can
generate a ULID as an ID value (consistent with Notepad's file naming). No
other special behavior is attached to it in v1.

---

## 9. Agenda

### 9.1 Scope

The agenda scans a user-specified folder for `.md` files containing
`outline: true` (or `overview`/`contents`) in their frontmatter. It collects:

- Headings with active TODO keywords
- Headings with SCHEDULED or DEADLINE timestamps

No persistent database. The scan runs on demand (or on app open if the folder
is small). Results are held in memory for the session.

### 9.2 Views

**TODO view** — lists all TODO-keyword headings across scanned files, grouped
by keyword, then by file.

**Agenda view** — a day-by-day display of SCHEDULED and DEADLINE items for a
configurable date range (default: 7 days centered on today). Overdue items
surface at the top.

### 9.3 Interaction

Clicking any item in agenda view opens the source file and scrolls/focuses the
relevant heading.

### 9.4 Scan Folder

Configured in app settings. A single folder, non-recursive by default
(recursive scan is opt-in). This is the only cross-file feature in v1.

---

## 10. Keybindings (Outline Mode Only)

| Key | Action |
|---|---|
| `TAB` (on heading) | Cycle fold state: FOLDED → CHILDREN → SUBTREE |
| `TAB` (in body text) | Normal indent |
| `TAB` (on property drawer) | Toggle drawer expanded/collapsed |
| `S-TAB` | Global fold cycle: OVERVIEW → CONTENTS → SHOWALL |
| `M-↑` | Move section up (swap with previous sibling) |
| `M-↓` | Move section down (swap with next sibling) |
| `M-←` | Promote heading one level |
| `M-→` | Demote heading one level |
| `M-S-←` | Promote subtree one level |
| `M-S-→` | Demote subtree one level |
| `M-RET` | Insert new heading at same level |
| `C-c C-t` | Cycle TODO keyword on heading |
| `C-c C-s` | Set SCHEDULED timestamp |
| `C-c C-d` | Set DEADLINE timestamp |

All `M-` and `C-c` bindings are no-ops in document mode.

---

## 11. Read Mode

In outline mode, read mode always renders SHOWALL — fold state does not carry
into the rendered view. TODO keywords, tags, SCHEDULED/DEADLINE timestamps, and
property drawers are rendered with appropriate styling.

`toc: true` continues to work independently of `outline`.

---

## 12. Architecture

**Fork of `mdview` (`C:\Users\cduff\mdview`), not Notepad.**

mdview is a Tauri 2 desktop app (Windows-native) that already provides the
core infrastructure this outliner needs. Windows-only is an accepted constraint.

### What mdview already provides

- **Tauri 2 + Rust backend** — native file I/O with no dialog prompts, atomic
  writes, file watching with debounce, dirty state tracking. All the plumbing
  that a browser app would need workarounds for is already solved.
- **CodeMirror 6** — already integrated with markdown syntax highlighting,
  live preview, and a search/replace panel. The keybinding layer for Org
  commands goes on top of what's already there.
- **`md-engine` crate (comrak-based)** — markdown parser that returns heading
  metadata with exact source positions (`line_start`, `line_end`). This is
  the foundation for fold/move operations. Extend this crate to parse TODO
  keywords, tags, timestamps, and property drawers.
- **TOC sidebar** — already extracts a heading tree and handles click-to-jump.
  The outline panel is this, evolved into an interactive fold/move surface.
- **Windows shell integration** — file associations, context menu, Explorer
  preview pane. The outliner can register `.md` files as its own type.
- **Settings storage** — `%APPDATA%\mdview\config.json` pattern already
  established. Global default TODO keywords and scan folder config live here.

### What gets added

- Org-like frontmatter parsing (the `outline`, `todo`, `done`, `tags` keys)
- TODO keyword parsing and state cycling in `md-engine`
- Tag parsing (`## Heading :tag:`) in `md-engine`
- SCHEDULED/DEADLINE timestamp parsing in `md-engine`
- Property drawer parsing in `md-engine`
- Outline keybindings in CodeMirror (TAB fold, M-arrows, C-c chords)
- Fold state management in the frontend (ephemeral, per-session)
- Gutter fold indicators on heading lines
- Interactive outline panel (fold/unfold, drag-to-reorder backed by M-arrows)
- Agenda view (folder scan, TODO + scheduled items aggregated)
- Date picker for SCHEDULED/DEADLINE insertion

### What does not change from mdview

- Tauri 2 app shell and window boot sequence
- Atomic file write and file watcher infrastructure
- Live preview rendering pipeline
- Search/replace panel
- Theme system (CSS variables, Windows accent sync)

No backend. No sync. No accounts. Files remain plain `.md` text.

---

## 13. Explicitly Out of Scope (v1)

- Tag inheritance
- Repeating timestamps (`+1w`, `.+1d`, etc.)
- Multiple TODO keyword sequences per file
- Org tables with formula support
- Org-babel (executable code blocks)
- Column view
- Time clocking (`:LOGBOOK:`)
- Export to PDF or LaTeX
- System-wide capture via tray (possible in Tauri but deferred)
- Backlinks, graph view, or any cross-file index beyond the agenda scan
- Inline images or attachments

---

## 14. Open Questions

These are genuine ambiguities requiring a decision before implementation.

**Q1. Demote clamping at H6.**
If promoting/demoting a subtree would push a child heading beyond `######`,
should the command be a no-op (safest), or clamp children at H6 and proceed
anyway?

**Q2. M-RET in a folded section.**
If a heading is in FOLDED state and you press M-RET, should the new heading be
inserted after the folded section (treating the whole section as a unit), or
should the heading unfold first?

**Q3. App-level default TODO keywords.**
Should the global default (`TODO | DONE`) be user-configurable in app settings,
or hardcoded? If configurable, where does that config live (a settings file in
the scan folder? localStorage?)?

**Q4. Agenda scan: recursive or not by default?**
Non-recursive is safer for large folders but requires the user to keep
everything flat or configure explicitly. Recursive with a depth limit (e.g. 3)
might be more useful in practice.

**Q5. What does the fold indicator look like for a heading with no foldable
content?**
A heading with only body text (no child headings) can still be folded. Does it
show an indicator? A different indicator? Or none (since it can't cycle through
CHILDREN state)?
