# Meye — Design

**Date:** 2026-05-24
**Status:** Draft for review
**Platform:** macOS (Apple Silicon, arm64)
**Builds on:** the existing "Screenpipe Keeper" app (launchd keeper) in this repo.

## Vision

**Meye** ("Me" + "Eye", reads like "my") is a local, private personal-memory app built on
[screenpipe](https://github.com/mediar-ai/screenpipe). It keeps screenpipe recording 24/7 (today's
keeper), surfaces screenpipe's **entire local API** in a usable UI, and grows an AI recall layer and
Obsidian organization on top. Everything runs locally; no data leaves the machine except, in Phase 2,
explicit calls the user makes to the Anthropic API.

Bundle identity: app = `com.meye.app`, recorder = `com.meye.recorder`.

## Why phased

The end goal is large (full client + AI chat + Obsidian). Building it as one blob is risky, so it is
decomposed into phases. Each phase is independently shippable and gets its own implementation plan.
**This spec details Phase 0 and Phase 1** (what we build next) and sketches Phases 2–3 as roadmap.

| Phase | Outcome | Status |
|-------|---------|--------|
| 0 | Rebrand to Meye + signed `.app` bundle for the recorder → **mic permission works** | plan now |
| 1 | Full screenpipe REST API wrapped + usable UI (Search, Devices/Recording, Tags) | plan now |
| 2 | Claude chat over your recordings (Anthropic API + screenpipe as tools) | future spec |
| 3 | Obsidian organization (summaries/digests into the vault) | future spec |

The existing keeper (launchd control, /health, pin/update) becomes the **Status** tab.

---

## Phase 0 — Rebrand + signed recorder bundle (the mic fix)

### Problem
The LaunchAgent runs the bare `screenpipe` binary. A raw Mach-O binary has no `Info.plist`, so it
cannot declare microphone usage, and macOS never creates a togglable **Microphone** entry for it —
which is exactly why the user can't grant mic. screenpipe then "times out waiting for permissions"
and never opens port 3030.

### Fix: wrap the recorder in a signed `.app` bundle
Pin screenpipe as a minimal app bundle instead of a flat dir:

```
~/Library/Application Support/meye/Meye Recorder.app/
  Contents/
    Info.plist            # CFBundleIdentifier=com.meye.recorder, CFBundleExecutable=screenpipe,
                          # NSMicrophoneUsageDescription, LSUIElement=true
    MacOS/
      screenpipe          # the pinned binary (executable bit preserved)
      mlx.metallib        # MLX Whisper model — must sit next to the binary
    _CodeSignature/       # produced by ad-hoc codesign
```

- The LaunchAgent's `ProgramArguments[0]` becomes
  `…/Meye Recorder.app/Contents/MacOS/screenpipe` (arg `record`).
- Because the executable lives in a bundle whose `Info.plist` declares `NSMicrophoneUsageDescription`,
  macOS attributes TCC to `com.meye.recorder`: a Microphone entry appears as **"Meye Recorder"**,
  and the user can grant it. (Screen Recording is still granted via the `+` button as before.)
- `mlx.metallib` stays adjacent to the binary inside `Contents/MacOS/` so transcription still works.

### Signing
- **Now:** ad-hoc sign — `codesign --force --deep --sign - "Meye Recorder.app"`. The Microphone entry
  appears. **Caveat:** ad-hoc identity is not stable across some rebuilds, so macOS may reset the
  permission and re-prompt after an update. Acceptable for personal use.
- **Later (Developer ID):** change the signing identity from `-` to the user's Developer ID; the
  identity becomes stable and permissions survive updates. This is a one-argument change isolated in
  `binary.rs` (a `SIGNING_IDENTITY` constant) and `tauri.conf.json`.
- The main Meye app bundle (`com.meye.app`) is ad-hoc signed by `tauri build` for now (signingIdentity
  `-`), same later-swap path.

### Rebrand
- `tauri.conf.json`: `productName` = "Meye", `identifier` = `com.meye.app`.
- Cargo package/lib name → `meye` / `meye_lib`; `main.rs` calls `meye_lib::run()`.
- Constants: launchd `LABEL` → `com.meye.recorder.agent`; app data dir → `~/Library/Application Support/meye/`;
  logs → `~/Library/Logs/meye/`.
- README + window title updated. (Repo folder may be renamed to `meye` later; not required now.)

### Migration
On first run of the rebranded app, if the old `com.screenpipe.keeper` agent exists, bootout + remove
its plist and old pinned dir, then run the new setup (pin bundle + new agent). A one-time
`migrate_from_keeper()` handles this so the user isn't left with two agents.

### Phase 0 files
- `binary.rs` — `pin()`/`update()` build the `.app` bundle (write Info.plist, copy binary+metallib into
  `Contents/MacOS/`, ad-hoc codesign) instead of a flat copy. New pure helper
  `build_info_plist(bundle_id, exec_name) -> String` (unit-tested).
- `agent.rs` — `program_arguments()` points at the bundle executable; `LABEL` renamed; add
  `migrate_from_keeper()`.
- `paths.rs` — new dirs/labels (`meye`).
- `tauri.conf.json`, `Cargo.toml`, `main.rs` — rebrand.

---

## Phase 1 — Full screenpipe API, usable

### Goal
Expose **every** screenpipe REST endpoint through one typed Rust client and a tabbed UI, so the user
can actually use their captured data.

### screenpipe REST endpoints to wrap (`screenpipe_api.rs`)
Base `http://127.0.0.1:3030`.

| Method | Path | Use |
|--------|------|-----|
| GET | `/health` | status (already used) |
| GET | `/search` | content search; params `q, content_type(ocr\|audio\|ui\|all), limit, offset, start_time, end_time, app_name, window_name, browser_url, min_length, max_length` |
| GET | `/search/keyword` | keyword search |
| GET | `/frames/{id}` | frame image bytes (for thumbnails/preview) |
| GET | `/frames/{id}/ocr` | frame OCR text + bounds |
| GET | `/audio/list` | list audio devices |
| POST | `/audio/start` | start audio capture |
| POST | `/audio/stop` | stop audio capture |
| GET | `/vision/list` | list monitors |
| POST | `/tags/{type}/{id}` | add tags to a content row |
| DELETE | `/tags/{type}/{id}` | remove tags |
| POST | `/raw_sql` | advanced query (power-user panel) |
| POST | `/add` | add content (advanced; likely hidden in v1) |

Each gets a typed request/response struct (serde, tolerant `#[serde(default)]`). Pure
query-string/body builders are unit-tested; the HTTP calls are thin async wrappers (reqwest), not
unit-tested. `raw_sql`/`add` are exposed but behind an "Advanced" disclosure (YAGNI on heavy UI).

### Commands (`commands.rs`)
`api_search(params)`, `api_keyword(q, limit)`, `api_frame_ocr(id)`, `api_audio_devices()`,
`api_audio_start()`, `api_audio_stop()`, `api_monitors()`, `api_add_tag(kind,id,tags)`,
`api_remove_tag(kind,id,tags)`, `api_raw_sql(sql)`. Frame images are loaded by the webview directly
from `http://127.0.0.1:3030/frames/{id}` (no need to round-trip bytes through Rust).

### UI — tab shell
The window becomes a tabbed app. Tabs:
- **Status** — the existing keeper view (health dot, Start/Stop/Restart, Update, permission banner,
  log tail). Unchanged behavior.
- **Search** — query box + filters (content type, app, window, time range, limit). Results list: each
  row shows snippet/transcript text, timestamp, app/window, and a thumbnail `<img>` from
  `/frames/{id}` (for OCR/vision hits). Click a row → detail (full frame image + OCR text via
  `/frames/{id}/ocr`, or full transcript for audio). "Add tag" inline.
- **Devices / Recording** — list monitors (`/vision/list`) and audio devices (`/audio/list`); buttons
  for audio Start/Stop; surface current recording state from `/health`.
- **Advanced** (collapsible, optional) — raw SQL box → `/raw_sql` table view.

Frontend grows from one `main.ts` into per-tab modules: `tabs.ts` (shell + routing),
`status.ts` (existing logic moved), `search.ts`, `devices.ts`, `advanced.ts`, shared `api.ts`
(invoke wrappers). This keeps each file focused.

### Phase 1 files
- Create: `src-tauri/src/screenpipe_api.rs` (client + types + tested builders).
- Modify: `src-tauri/src/commands.rs` (new `api_*` commands), `lib.rs` (register them).
- Frontend: `index.html` (tab bar + tab panels), `src/api.ts`, `src/tabs.ts`, `src/status.ts`,
  `src/search.ts`, `src/devices.ts`, `src/styles.css`.

---

## Phase 2 — Claude chat over recordings (roadmap)
New **Chat** tab. Rust `chat.rs`: Anthropic API client (with prompt caching) where screenpipe
`search`/`frame_ocr` are exposed as tool definitions; the model retrieves context to answer
("what did I work on this morning?", "find the error I saw at 3pm"). `ANTHROPIC_API_KEY` stored in
the macOS Keychain (never on disk in plaintext). Detailed in its own spec.

## Phase 3 — Obsidian organization (roadmap)
`obsidian.rs`: on demand or on a schedule, summarize screenpipe activity (e.g. an activity rollup +
LLM summary) into the user's Obsidian vault as daily/topic notes. Reuses Phase 2's LLM layer.
Detailed in its own spec.

---

## Cross-cutting

**Tech:** Tauri v2 + Rust backend + vanilla-TS frontend (existing). No framework added in Phase 0/1
(tabs are simple); revisit if Phase 2 chat UI warrants it.

**Error handling:** all `api_*` commands return `Result<_, String>` with humanized messages (reuse the
launchctl humanizer pattern). If 3030 is unreachable, the Search/Devices tabs show a "recorder not
running / waiting for permissions" empty state that links to the Status tab — never raw errors.

**Testing:**
- Phase 0: unit-test `build_info_plist` (contains bundle id, exec name, mic usage string) and the
  updated plist `ProgramArguments` path. Bundle creation + codesign verified manually.
- Phase 1: unit-test the pure query-string/body builders in `screenpipe_api.rs` (e.g. `/search`
  params with optional filters omitted/included; tag endpoints path building). HTTP + UI verified
  manually against the live instance.

**Out of scope (Phase 0/1):** cross-platform; multiple recorder profiles; editing screenpipe pipes;
the `/add` endpoint UI; in-app video export; auto-update of Meye itself; Developer ID signing (later).
