# NxMonitor — repository instructions

## Start here

Read `docs/STATUS.md` first, then `docs/PRODUCT.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, and `docs/TESTING.md`. Setup/another Windows machine: `docs/DEVELOPMENT.md`. Confirmed user scope: `docs/requirements.md`. Keep these artifacts current; do not rely on prior chat or a session scratch folder.

## Stack and current phase

- Rust edition 2024, MSVC x64, minimum Rust 1.96. GPUI-Kit pinned to 0.8.0; keep matching GPUI snapshots and Cargo.lock.
- Enable `--features hud` for desktop. Recommended bin: `nxmonitor-desktop`.
- User accepted the static Graphy visual direction and chose to continue GPUI-Kit. Do not migrate to Tauri/SolidJS or continue polishing the static sample without a new request.
- Real external FPS, target/settings, memory and basic device/display info implemented. Next: DEV-03 bounded long-session capture/statistics; audio, full visual integration, tray/startup and multi-display remain. Do not claim full v1.

## Invariants

- Non-injected/normal privileges first. Never auto-elevate, alter user groups or bypass anti-cheat/security restrictions.
- Presented FPS is not displayed FPS; UI refresh is not target FPS. Static sample values must never masquerade as live data.
- Target identity = PID + creation time. Do not persist/relock runtime PIDs. Automatic mode clears manual lock; manual blacklist override requires confirmation.
- Never mix target/swapchain sessions. Reject stale results after target change.
- Atomic settings save + reserved command publication must stay consistent. Invalid config is preserved and must not start automatic capture or be silently overwritten.
- Own/stop only the helper and unique ETW session created by this instance. Do not terminate all PresentMon processes or default trace sessions.
- Keep system queries, parsing, processes and persistence out of render. Tasks/subscriptions have explicit lifecycle owners; use weak Entity updates where appropriate.
- One Root per GPUI window; use gpui_kit facade, not guessed web/old-GPUI APIs. Read relevant local guides/component source before UI edits. Theme tokens/rem for ordinary layout; document canvas/platform physical-coordinate exceptions.
- Existing 600s/per-chain sample limits are prototype constraints. Do not just remove them without bounded storage/long-session tests.

## Checks (Windows PowerShell, repo root)

```powershell
cargo fmt --all -- --check
cargo test --locked --features hud --all-targets -j 1
cargo clippy --locked --features hud --all-targets -- -D warnings
cargo build --locked --features hud --bin nxmonitor-desktop -j 1
```

Test affected boundaries early; report automated vs native/manual evidence separately. Latest baseline: 32 passing tests, not a fixed quota. UI/ETW tests need an interactive authorized desktop; unit tests do not validate Windows11/fullscreen/mixed-DPI/release performance. Use isolated --config, preserve default settings, check actual UIA bounds and scroll before actions.

## Workspace and collaboration

- Preserve unrelated changes. Check git status when a repo exists; do not reset/clean user work. Do not init/push/create GitHub resources without user authorization.
- Do not commit target/, helper EXE/downloads, personal settings, secrets or traces. Keep Cargo.lock, source, docs, font files and licenses. PresentMon is obtained with `scripts/Get-PresentMon.ps1`.
- No source/build dependency on old `D:\Code\graphy` or any assistant scratch directory. Raw original reference images were not archived; existing screenshot evidence is historical, not a pixel-perfect golden.
- Finish a bounded task with relevant tests and updated STATUS/ROADMAP/validation evidence. Do not turn speculative risks into blockers or expand scope into postponed v1 features.
