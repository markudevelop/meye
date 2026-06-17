# Default pipe seeding — design

Date: 2026-06-17

## Problem

screenpipe "pipes" (scheduled LLM automations defined by a `pipe.md`) live in
`~/.screenpipe/pipes/<name>/` — per-machine runtime state. They are not part of
the meye app binary or git repo, so a fresh install (notably a second machine,
e.g. Windows) ships with **no pipes**. The user has a working set on macOS and
wants the same set present by default on every install.

The Pipes tab in the app (`src/pipes.ts` + `src-tauri/src/pipes.rs`) already
lists, runs, enables/disables, schedules, edits, logs, deletes, and
registry-installs pipes. The only gap is that nothing seeds a default set on
first run, and `obsidian-sync` hardcodes a machine-specific Obsidian vault path.

## Goal

On a fresh install of any OS, meye seeds the full set of pipes the user runs
today, accurately reflected in the Pipes tab, with the Obsidian vault path
configured once during onboarding instead of hardcoded.

## Pipes in scope (10)

`ai-habits`, `day-recap`, `meeting-summary`, `monthly-summary`, `obsidian-sync`,
`standup-update`, `time-breakdown`, `video-export`, `weekly-summary`,
`yearly-summary`.

## Components

### 1. Bundled pipe snapshots — `src-tauri/resources/pipes/<name>/`
Snapshot each pipe's **definition only** — `pipe.md`, `.pi/`, and
`.screenpipe-permissions.json` — excluding `logs/` and `output/` (runtime
artifacts). Total ≈ 400K. Bundled into the app via `tauri.conf.json`
`bundle.resources`. Any machine-specific absolute path found in a `pipe.md`
(the `/Users/.../Obsidian Vault iCloud/...` path in `obsidian-sync`; all
pipe.md files are scanned, not just that one) is replaced in the snapshot with
a `{{VAULT}}` placeholder.

### 2. First-run seeding — `pipes::seed_defaults()`
Called from `setup()` in `lib.rs`. For each bundled pipe: if
`~/.screenpipe/pipes/<name>` is **absent**, copy the snapshot in. Then
substitute the configured vault path into any `{{VAULT}}` placeholder in the
copied `pipe.md`.

- **Idempotent and non-destructive:** an existing pipe directory is left
  untouched — never overwrite user edits, logs, or output. Safe to run on every
  launch.
- Resolves the resource dir via Tauri's resource resolver; resolves the target
  via existing `paths::` helpers.

### 3. Vault path config + onboarding step
- New pref `obsidian_vault` (stored alongside existing prefs).
- Onboarding gains one step: "Where's your Obsidian vault?", pre-filled with an
  auto-detected default per OS:
  - macOS: `~/Library/Mobile Documents/com~apple~CloudDocs/Obsidian/Obsidian Vault iCloud`
  - Windows: `%USERPROFILE%\iCloudDrive\Obsidian`
  - else: `~/Documents/Obsidian`
- A Settings field updates it later; changing it rewrites the `{{VAULT}}` target
  in `obsidian-sync/pipe.md` (re-substitute from the bundled snapshot's template
  line so repeated edits don't corrupt the file).

### 4. Pipes tab accuracy
Seeded pipes live in the standard dir, so `pipeList` surfaces them
automatically. Verify enabled/schedule/last-run render correctly for seeded
pipes (last-run is already enriched from `logs/` because screenpipe reports it
as null). No new UI required.

### 5. Cross-platform
All seeding uses `paths::` helpers and the placeholder substitution, keeping
everything OS-agnostic. No hardcoded separators or user paths in seeded output.

## Testing
Unit tests:
- seed copies a missing pipe,
- seed skips an existing pipe (non-destructive),
- `{{VAULT}}` substitution produces the configured path,
- vault default detection per `cfg!(target_os)`.

Existing 31 unit tests stay green.

## Out of scope (YAGNI)
- Live pipe sync across machines.
- Editing bundled snapshots from the UI; per-pipe "reset to default" / badge.
- Per-pipe version migration of already-installed pipes.
