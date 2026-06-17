# Remote meeting-audio in the viewer — design

Date: 2026-06-17

## Problem

The remote viewer (macOS, in our case) shows the host's screen and lets the
local AI narrate/answer using the host's **OCR text only** (`remote_search` is
hardcoded `content_type=ocr`). In a meeting on the host (Windows), people speak —
the host's **system audio** carries the other participant's voice and the host's
**mic** carries whoever is at the host. None of that reaches the viewer's AI, so
it cannot help with spoken questions.

## Goal

The viewer's AI factors in the host's recent **audio transcript** (whatever
streams are active — system audio and/or mic) alongside the on-screen text, so it
can respond to questions asked aloud in a meeting.

## Design

### Host side (config, not code)
Audio capture + Whisper transcription must be on, producing `audio_transcriptions`
rows. meye's default captures **system audio only**; for meetings the user
enables the **mic** too in Performance → Capture sources on the host. "Capture
whatever is active" — the viewer makes no assumption about which streams exist; it
just reads whatever transcriptions are present.

### Viewer side (code)

1. **Generalize `remote_search`** (`screenpipe_api.rs`) to take a `content_type`
   parameter instead of the hardcoded `"ocr"`. Existing callers pass `"ocr"`.
   Add a Tauri command `api_remote_audio(host, token, since, limit)` that queries
   `content_type=audio`.

2. **`remote.ts` pulls audio each tick** (reusing the existing host-clock-skew
   window so it stays cheap): a second lightweight query for recent audio
   transcriptions in the narrow window, limit ~10. Build a `recentAudio` string
   from the returned rows (most recent last), including a speaker label when the
   row carries one (screenpipe diarization), e.g. `Speaker 1: …`.

3. **Combined AI context.** `maybeNarrate()` / `ask()` send a context that
   merges screen + audio instead of just `recentOcr`:
   ```
   SCREEN:
   <recent OCR>

   MEETING AUDIO (most recent last):
   <transcript lines>
   ```
   `remoteComment(context, question)` is unchanged — it already takes a free-form
   context string.

4. **Narration trigger.** Today narration fires on a new frame. Add: also fire
   when new audio arrives (new max audio timestamp) even if the frame is
   unchanged — meetings can have a static screen while people talk. Keep the
   existing `NARRATE_MS` (12s) throttle so it doesn't spam.

### Out of scope (YAGNI)
- Viewer-side voice input (asking the AI by speaking into the Mac's mic) —
  separate feature; the "Ask" box stays text for now.
- Real-time streaming transcription / sub-second audio latency. Polling the
  host's transcriptions at the existing cadence is enough for Q&A assist.
- Speaker identification beyond whatever label screenpipe already attaches.

## Testing
- `remote_search` with `content_type=audio` builds the expected query pairs.
- Context assembly merges OCR + audio and omits empty sections.
- Existing remote/unit tests stay green.
