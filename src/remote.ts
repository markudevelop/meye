import { $, toast } from "./ui";
import { api } from "./api";
import { renderMarkdown } from "./md";

// Remote viewer: connect to another Meye host on the LAN, show its live screen (the
// frames it already captures) and let the LOCAL AI narrate/help via the host's OCR text.
// Read-only — we only read the host's frames + OCR, never control it.

const POLL_MS = 1500; // how often we ask the host for its latest frame
const NARRATE_MS = 12000; // min gap between automatic AI narration calls
// "Latest frame" window. The host filters by start_time against ITS clock, which can differ from
// this viewer's by hours (timezone/skew/UTC-vs-local). So: cast a wide net only until we've seen
// one frame, then anchor a NARROW window to the host's own clock (learned from that frame's
// timestamp). Narrow keeps each poll cheap on the host — a fixed 24h window made every 1.5s poll a
// heavy whole-day scan that could overload (and degrade) the host's recorder.
const FIRST_WINDOW_MS = 24 * 60 * 60 * 1000; // clock-agnostic, used only until the first frame
const NARROW_WINDOW_MS = 15 * 60 * 1000; // light steady-state window, anchored to host time
let hostSkewMs: number | null = null; // (host frame time) − (this viewer's clock)

let host = "";
let token = "";
let pollTimer: ReturnType<typeof setInterval> | null = null;
let lastFrameId: number | null = null;
let lastNarrateAt = 0;
let narrateBusy = false;
let recentOcr = "";
let recentAudio = ""; // recent host audio transcript (meeting speech), newest last
let lastAudioTs = ""; // newest audio timestamp seen — detects fresh speech
let ticking = false; // guard so a slow poll never overlaps the next

// What the AI sees: the host's on-screen text plus any recent meeting audio. Either may be
// empty (screen-only host, or a static screen with people talking) — empty sections are dropped.
function buildContext(): string {
  const parts: string[] = [];
  if (recentOcr) parts.push("SCREEN:\n" + recentOcr);
  if (recentAudio) parts.push("MEETING AUDIO (most recent last):\n" + recentAudio);
  return parts.join("\n\n");
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
}
function sinceIso(): string {
  const base = hostSkewMs === null ? Date.now() - FIRST_WINDOW_MS : Date.now() + hostSkewMs - NARROW_WINDOW_MS;
  return new Date(base).toISOString();
}

function setStatus(text: string, live: boolean) {
  $("rv-status").innerHTML = `<span class="live-pulse${live ? "" : " off"}"></span> ${esc(text)}`;
}

// --- host side: expose this machine ---
async function refreshHostCard() {
  const toggle = $("rv-host-toggle") as HTMLInputElement;
  const info = $("rv-host-info");
  try {
    const p = await api.remotePairing();
    toggle.checked = p.enabled;
    if (p.enabled) {
      info.classList.remove("hidden");
      info.innerHTML =
        `<div class="rv-pair"><span class="rv-pair-label">Address</span><code>${esc(p.host)}</code></div>` +
        `<div class="rv-pair"><span class="rv-pair-label">Pairing code</span><code class="rv-code">${esc(p.token)}</code></div>` +
        `<div class="meta">On the viewing device, open <b>Remote</b> and enter these. Only devices on your network that have this code can view.</div>`;
    } else {
      info.classList.add("hidden");
      info.innerHTML = "";
    }
  } catch {
    /* pairing needs the recorder running; leave the toggle as-is */
  }
}

function initHostCard() {
  const toggle = $("rv-host-toggle") as HTMLInputElement;
  toggle.onchange = async () => {
    const on = toggle.checked;
    toggle.disabled = true;
    try {
      toast(on ? "Enabling remote viewing — restarting recorder…" : "Disabling remote viewing…");
      await api.setRemoteEnabled(on);
      await refreshHostCard();
      toast(on ? "This machine is now viewable on your network" : "Remote viewing disabled");
    } catch (e) {
      toast(`Failed: ${String(e)}`);
      toggle.checked = !on;
    } finally {
      toggle.disabled = false;
    }
  };
}

// --- viewer side: watch another machine ---
function showError(msg: string) {
  const e = $("rv-error");
  e.textContent = msg;
  e.classList.remove("hidden");
}

async function connect() {
  host = ($("rv-host") as HTMLInputElement).value.trim();
  token = ($("rv-token") as HTMLInputElement).value.trim();
  $("rv-error").classList.add("hidden");
  if (!host || !token) {
    showError("Enter both an address and a pairing code.");
    return;
  }
  const btn = $("rv-connect") as HTMLButtonElement;
  btn.disabled = true;
  try {
    await api.remoteLatest(host, token, sinceIso()); // validate the connection
    localStorage.setItem("meye.remote.host", host);
    localStorage.setItem("meye.remote.token", token);
    lastFrameId = null;
    recentOcr = "";
    recentAudio = "";
    lastAudioTs = "";
    lastNarrateAt = 0;
    hostSkewMs = null;
    $("rv-ai-out").innerHTML = `<div class="meta">Narration appears here as the screen changes.</div>`;
    ($("rv-img") as HTMLImageElement).removeAttribute("src");
    $("rv-setup").classList.add("hidden");
    $("rv-live").classList.remove("hidden");
    setStatus("Connected", true);
    await tick();
    if (!pollTimer) pollTimer = setInterval(() => void tick(), POLL_MS);
  } catch (e) {
    showError(`Couldn't connect: ${String(e)}. Check the address & code, and that the other machine has remote viewing enabled.`);
  } finally {
    btn.disabled = false;
  }
}

function disconnect() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
  $("rv-live").classList.add("hidden");
  $("rv-setup").classList.remove("hidden");
}

async function tick() {
  if (ticking) return; // a previous poll is still in flight — skip this beat
  ticking = true;
  try {
    let res: any;
    try {
      res = await api.remoteLatest(host, token, sinceIso());
    } catch {
      setStatus("Reconnecting…", false);
      return;
    }
    setStatus("Connected", true);
    const hit = (res.data ?? [])[0];
    if (!hit) {
      $("rv-meta").textContent = "No frames from the host yet — is recording on over there?";
      return;
    }
    const c = hit.content ?? hit;
    const fid = c.frame_id ?? c.frameId ?? c.id;
    const text = String(c.text ?? c.ocr_text ?? "").trim();
    const app = c.app_name ?? "";
    const ts = c.timestamp ?? "";
    const tsMs = ts ? new Date(ts).getTime() : NaN;
    if (!isNaN(tsMs)) hostSkewMs = tsMs - Date.now(); // anchor the next window to the host's clock
    if (text) recentOcr = text;
    $("rv-meta").textContent = [app, ts ? new Date(ts).toLocaleTimeString() : ""].filter(Boolean).join(" · ");
    if (fid != null && fid !== lastFrameId) {
      lastFrameId = fid;
      try {
        ($("rv-img") as HTMLImageElement).src = await api.remoteFrame(host, token, fid);
      } catch {
        /* keep the last frame on a transient fetch error */
      }
      void maybeNarrate();
    }

    // Audio is OPT-IN (default off): the viewer is screen-only unless the user ticks "Include
    // audio". This keeps the host's mic / meeting speech off the wire and out of the AI context
    // by default — you choose to listen, per connection.
    if (($("rv-audio") as HTMLInputElement).checked) {
      try {
        const ares: any = await api.remoteAudio(host, token, sinceIso(), 10);
        const rows = (ares.data ?? []).map((h: any) => h.content ?? h);
        const lines = rows
          .map((c: any) => {
            const t = String(c.transcription ?? c.text ?? "").trim();
            if (!t) return "";
            const who = c.speaker?.name ?? (c.speaker_id != null ? `Speaker ${c.speaker_id}` : (c.device_name ?? ""));
            return who ? `${who}: ${t}` : t;
          })
          .filter(Boolean)
          .reverse(); // screenpipe returns newest-first; show newest last for the AI
        if (lines.length) recentAudio = lines.join("\n");
        const maxTs = rows.map((c: any) => String(c.timestamp ?? "")).filter(Boolean).sort().pop() ?? "";
        if (maxTs && maxTs !== lastAudioTs) {
          lastAudioTs = maxTs;
          void maybeNarrate(); // fresh speech — let the AI respond even if the screen didn't change
        }
      } catch {
        /* audio is optional; ignore when the host isn't transcribing */
      }
    } else if (recentAudio) {
      recentAudio = ""; // audio just turned off — drop it from the AI's context immediately
    }
  } finally {
    ticking = false;
  }
}

function appendAi(who: string, text: string, isUser = false) {
  const out = $("rv-ai-out");
  if (out.querySelector(".meta") && !out.querySelector(".rv-msg")) out.innerHTML = "";
  const div = document.createElement("div");
  div.className = "rv-msg" + (isUser ? " rv-msg-user" : "");
  div.innerHTML = isUser ? `<b>${esc(who)}</b> ${esc(text)}` : `<div class="md">${renderMarkdown(text)}</div>`;
  out.appendChild(div);
  out.scrollTop = out.scrollHeight;
}

async function maybeNarrate() {
  if (!($("rv-narrate") as HTMLInputElement).checked) return;
  if (narrateBusy || (!recentOcr && !recentAudio)) return;
  if (Date.now() - lastNarrateAt < NARRATE_MS) return;
  narrateBusy = true;
  lastNarrateAt = Date.now();
  // Coach mode: turn the passive narration into an active pair-programming guide that
  // solves what it sees. Off → a brief observation.
  const coach = ($("rv-coach") as HTMLInputElement).checked;
  const directive = coach
    ? "Act as my live pair-programming guide. In a few short, concrete steps (do this, then do that), tell me what to do next based on what's on screen and any meeting audio. If you see a code bug/error or a math/logic problem — on screen or asked aloud — solve it and show the corrected line or snippet. Keep it tight and actionable."
    : "";
  try {
    appendAi(coach ? "🧭" : "👁", await api.remoteComment(buildContext(), directive));
  } catch {
    /* transient — try again on the next frame */
  } finally {
    narrateBusy = false;
  }
}

async function ask() {
  const input = $("rv-ask") as HTMLInputElement;
  const q = input.value.trim();
  if (!q) return;
  input.value = "";
  appendAi("🧑 You:", q, true);
  try {
    appendAi("👁", await api.remoteComment(buildContext(), q));
  } catch (e) {
    appendAi("⚠️", `Couldn't get an answer: ${String(e)}`);
  }
}

/** Blow the AI panel up to fill the window so long code fixes are readable. */
function toggleFullscreen(on?: boolean) {
  const ai = $("rv-ai");
  const full = on ?? !ai.classList.contains("fullscreen");
  ai.classList.toggle("fullscreen", full);
  $("rv-expand").textContent = full ? "✕ Exit full screen" : "⤢ Full screen";
}

export function initRemote() {
  $("rv-connect").onclick = () => void connect();
  $("rv-disconnect").onclick = () => disconnect();
  $("rv-ask-btn").onclick = () => void ask();
  $("rv-expand").onclick = () => toggleFullscreen();
  document.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Escape") toggleFullscreen(false);
  });
  ($("rv-ask") as HTMLInputElement).addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") void ask();
  });
  initHostCard();
  ($("rv-host") as HTMLInputElement).value = localStorage.getItem("meye.remote.host") ?? "";
  ($("rv-token") as HTMLInputElement).value = localStorage.getItem("meye.remote.token") ?? "";
}

/** Called when the Remote tab opens: refresh host info, resume polling if connected. */
export function startRemote() {
  void refreshHostCard();
  if (!pollTimer && !$("rv-live").classList.contains("hidden")) {
    pollTimer = setInterval(() => void tick(), POLL_MS);
  }
}

/** Called when leaving the Remote tab: pause polling to save bandwidth. */
export function stopRemote() {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}
