# ADR 0001: Tauri Shell for Main App

## Status
Accepted

## Context
The product requires a Windows-native experience with fast startup, low overhead, and deep OS integration.

## Decision
Use Tauri as the main app shell.

## Consequences
- Pros:
  - Small distribution footprint relative to Electron.
  - Rust-native integration path for Windows APIs.
  - Better fit for startup and IO control.
- Cons:
  - More custom integration work for niche Windows features.
  - Requires careful coordination between Rust shell and frontend runtime.
