# Meye

**Meye** ("Me" + "Eye") is a local, private personal-memory app on
[screenpipe](https://github.com/mediar-ai/screenpipe): it keeps screenpipe recording 24/7, surfaces
screenpipe's full API in a usable UI, and grows AI recall + Obsidian organization on top.
Runs on **macOS** and **Windows**.

## What it does
- **Keeps screenpipe alive** — on macOS via a launchd agent (`KeepAlive` + `RunAtLoad`); on
  Windows via a Task Scheduler task (logon trigger + 1-minute keep-alive tick). Auto-restarts
  on crash, starts at login, survives logout/reboot and the app being closed.
- **Pins a stable copy** of the screenpipe binary so npx updates don't break the agent's path.
  "Update screenpipe" re-pins the latest and restarts.
- **Control panel:** tray status dot, Start/Stop/Restart, health dashboard, log tail, data folder.

## Architecture
The OS supervisor (launchd / Task Scheduler) owns the process; the app controls the supervisor
and reads `http://127.0.0.1:3030/health`.
See `docs/superpowers/specs/2026-05-24-screenpipe-keeper-design.md`.

| Module | Role |
|--------|------|
| `paths.rs` | paths + constants (per-platform) |
| `agent.rs` | shared record-args + permissions logic |
| `agent_macos.rs` | plist + launchctl |
| `agent_windows.rs` | Task Scheduler XML + schtasks + hidden VBS launcher |
| `health.rs` | /health parse + classify |
| `binary.rs` | pin/update binary |
| `commands.rs` | Tauri commands |
| `lib.rs` | tray + window + poll loop (entry: `run()`) |

## Develop
```bash
# one-time: Rust toolchain (macOS/Linux; on Windows use https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env"

npm install
npm run tauri dev      # run in dev
npm run tauri build    # produce a .app bundle (macOS) / NSIS installer (Windows)
cd src-tauri && cargo test   # unit tests
```

## First run
Open the dashboard (tray → Open Dashboard) → **Set up & start**.

- **macOS:** grant **Screen Recording** permission when prompted (the pinned binary is a new
  path, so the grant is re-requested once), then click **Restart** in the app.
- **Windows:** no screen-recording permission exists; if audio shows "not capturing", allow
  microphone access in Settings → Privacy → Microphone.

## Manage manually
macOS:
```bash
launchctl print gui/$(id -u)/com.meye.recorder.agent      # status
launchctl bootout gui/$(id -u)/com.meye.recorder.agent    # stop
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.meye.recorder.agent.plist  # start
```

Windows (PowerShell):
```powershell
schtasks /Query /TN MeyeRecorder /V /FO LIST   # status
schtasks /Change /TN MeyeRecorder /DISABLE; taskkill /F /T /IM meye-recorder.exe  # stop
schtasks /Change /TN MeyeRecorder /ENABLE; schtasks /Run /TN MeyeRecorder         # start
```

## License
Meye is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)** —
see [LICENSE](LICENSE). Third-party components (screenpipe and others) retain their
own licenses; see [NOTICE](NOTICE).
