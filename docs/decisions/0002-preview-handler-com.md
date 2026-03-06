# ADR 0002: Separate COM Preview Handler Product Surface

## Status
Accepted

## Context
Explorer Preview Pane support is required for a true Windows markdown viewer.
Preview handlers run in `prevhost.exe` and cannot depend on app process state.

## Decision
Implement Preview Pane support as a separate COM DLL crate (`win-preview-handler`) with shared rendering and style crates.

## Consequences
- Pros:
  - Isolates high-risk COM complexity from main app velocity.
  - Clear lifecycle and debugging boundary.
  - Supports independent milestone and release gating.
- Cons:
  - More packaging and registration complexity.
  - Requires strict shared crate boundaries for parity.

## Implementation Notes
- Use `windows-rs` for COM interface and registration plumbing.
- Prefer installer-driven registration over manual registry scripts.
