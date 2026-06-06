# Meye

**Meye** ("Me" + "Eye") is a local, private personal-memory app on
[screenpipe](https://github.com/mediar-ai/screenpipe): it keeps screenpipe recording 24/7, surfaces
screenpipe's full API in a usable UI, and grows AI recall + Obsidian organization on top.

## What it does
- **Keeps screenpipe alive** with a launchd agent (`KeepAlive` + `RunAtLoad`): auto-restarts on
  crash, starts at login, survives logout/reboot and the app being closed.
- **Pins a stable copy** of the screenpipe binary (+ `mlx.metallib`) so npx updates don't break
  the agent's path. "Update screenpipe" re-pins the latest and restarts.
- **Control panel:** tray status dot, Start/Stop/Restart, health dashboard, log tail, data folder.

## Architecture
launchd owns the process; the app controls launchd and reads `http://127.0.0.1:3030/health`.
See `docs/superpowers/specs/2026-05-24-screenpipe-keeper-design.md`.

| Module | Role |
|--------|------|
| `paths.rs` | paths + constants |
| `agent.rs` | plist + launchctl |
| `health.rs` | /health parse + classify |
| `binary.rs` | pin/update binary |
| `commands.rs` | Tauri commands |
| `lib.rs` | tray + window + poll loop (entry: `run()`) |

## Develop
```bash
# one-time: Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env"

npm install
npm run tauri dev      # run in dev
npm run tauri build    # produce a .app bundle
cd src-tauri && cargo test   # unit tests
```

## First run
Open the dashboard (tray → Open Dashboard) → **Set up & start**. Grant **Screen Recording**
permission when macOS prompts (the pinned binary is a new path, so the grant is re-requested once),
then click **Restart** in the app.

## Manage manually
```bash
launchctl print gui/$(id -u)/com.screenpipe.keeper      # status
launchctl bootout gui/$(id -u)/com.screenpipe.keeper    # stop
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.screenpipe.keeper.plist  # start
```

## License
Meye is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)** —
see [LICENSE](LICENSE). Third-party components (screenpipe and others) retain their
own licenses; see [NOTICE](NOTICE).
