# Screenpipe Keeper — Design

**Date:** 2026-05-24
**Status:** Approved (pending spec review)
**Platform:** macOS (Apple Silicon, arm64)

## Problem

[screenpipe](https://github.com/mediar-ai/screenpipe) currently runs via `npx screenpipe@latest record` inside a Terminal window. Closing the terminal kills the recorder. The user wants:

1. screenpipe to **stay running** without a terminal open (survive logout, reboot, crash).
2. A **GUI control panel** to see status and start/stop/update it.
3. A **project repo** they can continue developing.

## Current environment (observed)

- screenpipe **v0.3.345**, run via `npx screenpipe@latest record`.
- Real binary lives at an **npx content-hash path**:
  `~/.npm/_npx/<hash>/node_modules/@screenpipe/cli-darwin-arm64/bin/screenpipe`
  — the `<hash>` changes on every update, so the path is **not stable**.
- That `bin/` dir also contains **`mlx.metallib` (~88 MB)** — required by the binary for MLX Whisper transcription. Must travel with the binary.
- HTTP API + `/health` on `127.0.0.1:3030`.
- Data + DB in `~/.screenpipe`.
- **No** LaunchAgent currently installed.

## Architecture — two layers

The reliability ("always running") and the UI are deliberately separated so the recorder keeps running even when the GUI is closed.

### Layer 1 — launchd LaunchAgent (keep-alive, independent of the app)

- Plist: `~/Library/LaunchAgents/com.screenpipe.keeper.plist`
- `Label` = `com.screenpipe.keeper`
- `ProgramArguments` = `[<pinned screenpipe path>, "record", <flags...>]`
- `KeepAlive` = `true` → auto-restart on crash
- `RunAtLoad` = `true` → start at login / boot
- `StandardOutPath` = `~/Library/Logs/screenpipe-keeper/out.log`
- `StandardErrorPath` = `~/Library/Logs/screenpipe-keeper/err.log`
- `EnvironmentVariables` = minimal `PATH` + `HOME` so the binary resolves its resources.
- Runs the binary **directly** — no terminal, no tty, no node/npx wrapper.

### Layer 2 — Tauri app "Screenpipe Keeper" (control panel)

- **System tray icon**, color-coded from `/health` polling (every 5s):
  - green = healthy
  - yellow = degraded (e.g. transcription backlog — screenpipe reports `status: degraded` while still recording)
  - red = loaded but `/health` unreachable / down
  - grey = agent not installed (setup needed)
- **Tray menu:** Start · Stop · Restart · Open Dashboard · Open Data Folder · View Logs · Update screenpipe · Quit
- **Dashboard window** (web UI) showing parsed `/health`: uptime, capture FPS, audio status + devices, pending transcriptions, monitors, version, plus a live tail of the log files and the same control buttons.

## Pinned binary

- Pinned location: `~/Library/Application Support/screenpipe-keeper/bin/`
- Setup/Update copies the **entire** source `bin/` dir contents (binary **and** `mlx.metallib`) to the pinned location. Copying only the binary breaks transcription.
- Source for the copy: resolve via `npx --yes screenpipe@latest record --help`-style invocation to populate the npx cache, then locate `@screenpipe/cli-darwin-arm64/bin/`, then copy. (Update button re-runs this, replaces files, then `kickstart -k`.)

## Process control (modern launchctl, per-GUI domain)

`$UID` is the user id (501 here).

| Action  | Command |
|---------|---------|
| Start   | `launchctl bootstrap gui/$UID <plist>` |
| Stop    | `launchctl bootout gui/$UID/com.screenpipe.keeper` |
| Restart | `launchctl kickstart -k gui/$UID/com.screenpipe.keeper` |
| Status  | `launchctl print gui/$UID/com.screenpipe.keeper` + `/health` poll |

- **Stop** = `bootout` (unloads → `KeepAlive` will NOT relaunch).
- **Start** = `bootstrap` (loads → `RunAtLoad` starts it).
- **Restart** = `kickstart -k` (kill + restart, agent stays loaded).
- Run-at-login is inherent: while the agent is bootstrapped, `RunAtLoad` starts it each login. "Disable at login" = bootout.

## macOS TCC permissions gotcha (handled in setup)

macOS privacy permissions (Screen Recording, Accessibility, Microphone) are keyed on the **binary path**. Moving screenpipe to a new pinned path means macOS may treat it as a new binary and **re-prompt for Screen Recording** (which cannot be auto-granted) on first launchd run.

Mitigation:
- Setup wizard warns about this and deep-links to System Settings → Privacy & Security → Screen Recording / Accessibility.
- Dashboard surfaces a clear "Screen recording permission needed" state when `/health` shows frames not advancing despite the agent running.

## Rust backend modules (`src-tauri/src/`)

- **`agent.rs`** — plist XML generation + launchctl command construction. Pure functions (build args / build plist string) separated from execution → unit-testable.
- **`health.rs`** — `/health` HTTP client, typed response structs, JSON parsing, status classification (healthy/degraded/down).
- **`binary.rs`** — pin & update the screenpipe binary (locate npx cache, copy dir incl. metallib, verify).
- **`commands.rs`** — Tauri command handlers: `agent_status`, `agent_start`, `agent_stop`, `agent_restart`, `get_health`, `open_data_dir`, `open_logs`, `tail_logs`, `update_screenpipe`, `install_agent`, `is_installed`.
- **`main.rs`** — tray setup, dashboard window, 5s health-poll loop driving tray color, app lifecycle.

## Frontend (`src/`)

- Vite + plain TypeScript (Tauri default; minimal dependencies).
- Single dashboard view + setup state. Calls Rust via Tauri `invoke`.
- Structured to allow growing into search/timeline views later (out of scope for v1).

## Data flow

1. **First run / not installed:** app detects no plist + no pinned binary → Setup state. User clicks Setup → `binary.rs` pins binary → `agent.rs` writes plist → `bootstrap`. Warn about TCC.
2. **Normal:** `main.rs` polls `/health` every 5s → updates tray color + pushes to dashboard if open.
3. **Controls:** buttons → Tauri commands → launchctl via `std::process::Command`.
4. **Update:** Update button → `binary.rs` fetches latest via npx → copies files → `kickstart -k` → re-poll.

## Error handling

| Condition | Behavior |
|-----------|----------|
| Agent not installed | "Setup" state, controls hidden except Setup |
| Pinned binary missing | Disable Start/Restart, prompt re-setup |
| `/health` unreachable but agent loaded | Red tray, "starting or down" + link to logs |
| Port 3030 already in use | Surface error from logs, suggest existing instance |
| Update network failure | Keep existing pinned binary, show error toast, agent untouched |
| launchctl non-zero exit | Show stderr in a toast / log panel |

## Testing

- **Rust unit tests** on pure logic:
  - plist XML generation matches expected structure for given inputs.
  - launchctl arg vectors built correctly for start/stop/restart/status.
  - `/health` JSON → typed struct parsing, incl. the degraded sample captured from the live instance.
  - status classification (healthy / degraded / down) from parsed health.
- **launchd side-effects** verified manually (load/unload/kickstart can't run meaningfully in CI).
- **Frontend** kept thin; manual verification of dashboard rendering + button wiring.

## Repo layout (`~/git/screenpipe-keeper`)

```
screenpipe-keeper/
  src-tauri/
    src/
      main.rs
      agent.rs
      health.rs
      binary.rs
      commands.rs
    icons/
    tauri.conf.json
    Cargo.toml
  src/
    index.html
    main.ts
    dashboard.ts
    styles.css
  package.json
  README.md
  docs/superpowers/specs/2026-05-24-screenpipe-keeper-design.md
```

## Out of scope (v1, YAGNI)

- In-app search / timeline / OCR browsing (screenpipe's own UI + API cover this).
- Cross-platform (Linux/Windows) — macOS-only for now.
- Multiple named profiles / multiple screenpipe instances.
- Auto-update of the Keeper app itself.
- Configurable screenpipe record flags UI — v1 uses a sensible default flag set; flags editable in the plist by hand.
