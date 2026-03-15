# Polish And Finish Roadmap (2026-03-15)

## Current State
- `v0.1.0-beta.2` is out.
- Explorer Preview Pane now renders markdown through WebView2 and is visually good enough for beta use.
- File open in the full `mdview` app is working.
- Viewer external links are already handled in the app.
- Main remaining polish issues are link behavior in Explorer preview, resize smoothness, and final installed-build confidence.

## Release Goal
Ship a polished Windows beta that is stable in Explorer, predictable about links, and validated from the packaged installer path rather than only repo-local registration.

## Recommended Order
1. Test and troubleshoot link behavior in Explorer preview.
2. Lock the final Explorer preview link policy for this release.
3. Smooth the most visible preview interaction rough edges.
4. Run full installed-build validation.
5. Decide whether to cut a broader beta release or do one last hardening pass.

## Workstream 1: Explorer Preview Links

### Goal
Make link behavior in the Explorer preview intentional and stable instead of accidental.

### What to test first
- Standard markdown external links:
  - `[label](https://example.com)`
  - `[label](mailto:test@example.com)`
- Internal heading links:
  - `[jump](#section-name)`
- Mixed content cases:
  - links inside list items
  - links near headings
  - malformed or partial links
- Non-standard vault/wiki links:
  - `[[Wiki Link]]`
  - bare URLs

### Questions to answer
- Are standard markdown links actually rendered as `<a>` in the Explorer preview?
- If clicked, do external links:
  - open externally,
  - stay inert,
  - or attempt WebView2 navigation inside the preview?
- If clicked, do internal `#heading` links:
  - scroll in-preview,
  - do nothing cleanly,
  - or destabilize the preview?
- Are non-standard vault links intentionally unsupported, or do we want a fallback rendering rule later?

### Preferred release policy
- External `http:`, `https:`, and `mailto:` links should not navigate inside the Explorer preview.
- Internal `#heading` links should either:
  - scroll inside the preview cleanly, or
  - be inert in a clearly consistent way.
- Unsupported link formats such as `[[wikilink]]` can remain plain text for now if they do not break rendering.

### Likely implementation follow-up
- Add a WebView2 navigation interception/cancel path in `crates/win-preview-handler/src/lib.rs`.
- Log link/navigation attempts in the preview log while testing.
- Decide whether external links should be:
  - cancelled and ignored, or
  - cancelled in-WebView and forwarded to the system browser.

### Exit criteria
- Clicking links never hangs Explorer.
- Clicking links never leaves the preview blank or broken.
- The behavior is documented and repeatable for:
  - external links,
  - internal `#heading` links,
  - unsupported vault-style links.

## Workstream 2: Explorer Preview Interaction Polish

### Goal
Reduce the rough edges that are visible now that the core WebView2 path works.

### Targets
- Resize smoothness:
  - investigate why pane drag feels visually jerky,
  - verify whether the jerk is controller resize frequency, layout thrash, or repaint timing.
- First-paint polish:
  - confirm there is no white flash or stale content flash during file switch.
- Large-file behavior:
  - verify truncation/readability on large markdown files,
  - confirm Explorer remains responsive.

### Exit criteria
- Preview still passes `docs/PREVIEW_REGRESSION_CHECKLIST.md`.
- Resize remains functional and looks acceptable for release.

## Workstream 3: Installed-Build Validation

### Goal
Prove that the packaged installer path is as good as the repo-local/dev-registration path.

### Required checks
- Package installer.
- Silent install/uninstall smoke.
- `--register` / `--unregister` from installed location.
- Preview Pane using installed `win_preview_handler.dll`.
- Normal `.md` file open using installed `viewer-shell.exe`.
- No localhost/dev-url regressions.

### Primary docs
- `docs/INSTALLER_RUNTIME_RUNBOOK.md`
- `docs/POST_WEBVIEW2_VALIDATION.md`
- `docs/PREVIEW_REGRESSION_CHECKLIST.md`

### Exit criteria
- Installer deploys the correct binaries.
- Preview and file-open behavior both work from installed location.
- Uninstall leaves the system in a sane state.

## Workstream 4: Release Decision

### If links are stable and installer validation passes
- Treat `v0.1.0-beta.2` as a solid public beta milestone.
- Optionally cut a small follow-up beta only if link policy changes require code changes.

### If links are unstable or installer validation fails
- Do one more hardening patch round before wider release.
- Keep the patch narrowly scoped to:
  - preview link policy,
  - installer/register/unregister behavior,
  - visible preview interaction issues.

## Immediate Next Session Checklist
1. Use a markdown file with standard links and confirm whether the preview actually renders them as clickable anchors.
2. Click one external link and one internal `#heading` link in Explorer preview.
3. Observe:
   - browser launch or no launch,
   - preview stability,
   - file-switch stability afterward.
4. If navigation is happening inside the preview or behavior is inconsistent, patch navigation interception next.
5. After link policy is fixed, run the installed-build validation runbook end to end.
