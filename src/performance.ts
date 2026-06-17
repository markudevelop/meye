import { $, wrap } from "./ui";
import { api } from "./api";

// --prioritize-input-latency keeps the mouse/keyboard responsive (yields CPU to input,
// skips a11y capture right after a click). --transcription-mode batch + --filter-music cut
// GPU/heat by transcribing in larger chunks and skipping music/video audio.
const PRESETS: Record<string, string[]> = {
  saver: [
    "--audio-transcription-engine",
    "whisper-tiny-quantized",
    "--transcription-mode",
    "batch",
    "--filter-music",
    "--prioritize-input-latency",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "120000",
  ],
  balanced: [
    "--audio-transcription-engine",
    "whisper-large-v3-turbo-quantized",
    "--transcription-mode",
    "batch",
    "--filter-music",
    "--prioritize-input-latency",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "60000",
  ],
  performance: ["--audio-transcription-engine", "whisper-large-v3-turbo", "--prioritize-input-latency"],
};

const AUDIO: Record<string, string[]> = {
  tiny: ["--audio-transcription-engine", "whisper-tiny-quantized"],
  balanced: ["--audio-transcription-engine", "whisper-large-v3-turbo-quantized"],
  best: ["--audio-transcription-engine", "whisper-large-v3-turbo"],
};

/** Strip all audio source flags (devices / disable / system-default) so we can rebuild them cleanly. */
function stripAudioSources(args: string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--disable-audio" || args[i] === "--use-system-default-audio") continue;
    if (args[i] === "--audio-device") {
      i++;
      continue;
    }
    out.push(args[i]);
  }
  return out;
}

function setVideoArg(args: string[], on: boolean): string[] {
  const without = args.filter((a) => a !== "--disable-vision");
  return on ? without : [...without, "--disable-vision"];
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
}

function devTitle(name: string): string {
  if (/output|system audio/i.test(name)) return "🔊 " + name.replace(/\s*\(output\)/i, "");
  return "🎙 " + name.replace(/\s*\(input\)/i, "");
}

/** Render one toggle per audio device from /audio/list, reflecting the current --audio-device selection. */
function renderAudioDevices(devs: any, args: string[]) {
  const list: any[] = Array.isArray(devs) ? devs : (devs?.data ?? []);
  const el = $("cap-audio");
  if (!list.length) {
    el.innerHTML = "<p class='meta'>No audio devices found.</p>";
    return;
  }
  const disabled = args.includes("--disable-audio");
  const selected: string[] = [];
  for (let i = 0; i < args.length; i++) if (args[i] === "--audio-device") selected.push(args[i + 1] ?? "");
  el.innerHTML = "";
  for (const d of list) {
    const name = String(d?.name ?? "");
    if (!name) continue;
    const on = !disabled && selected.includes(name);
    const row = document.createElement("div");
    row.className = "toggle-row";
    row.innerHTML =
      `<div class="tr-text"><div class="tr-title">${esc(devTitle(name))}</div>` +
      `<div class="tr-desc">${esc(name)}${d.is_default ? " · system default" : ""}</div></div>` +
      `<label class="switch"><input type="checkbox"${on ? " checked" : ""} /><span class="slider"></span></label>`;
    const cb = row.querySelector("input") as HTMLInputElement;
    cb.onchange = () => void toggleDevice(name, cb.checked);
    el.appendChild(row);
  }
}

/** Add/remove one audio device from the capture set, then apply. No devices => audio off. */
async function toggleDevice(name: string, on: boolean) {
  const cur = await api.getRecordArgs().catch(() => [] as string[]);
  let sel: string[] = [];
  for (let i = 0; i < cur.length; i++) if (cur[i] === "--audio-device") sel.push(cur[i + 1] ?? "");
  sel = sel.filter((d) => d !== name);
  if (on) sel.push(name);
  const base = stripAudioSources(cur).filter((a) => a !== "--disable-audio");
  if (sel.length === 0) {
    base.push("--disable-audio");
  } else {
    if (!base.includes("--audio-transcription-engine")) base.push("--audio-transcription-engine", "whisper-large-v3-turbo");
    for (const d of sel) base.push("--audio-device", d);
  }
  void applyArgs(base);
}

function stripAudio(args: string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--disable-audio") continue;
    if (args[i] === "--audio-transcription-engine") {
      i++; // also skip its value
      continue;
    }
    out.push(args[i]);
  }
  return out;
}

function currentAudioKey(args: string[]): string {
  const i = args.indexOf("--audio-transcription-engine");
  const v = i >= 0 ? args[i + 1] : "";
  if (v === "whisper-tiny-quantized") return "tiny";
  if (v === "whisper-large-v3-turbo") return "best";
  if (v === "whisper-large-v3-turbo-quantized") return "balanced";
  return "tiny"; // audio off / unknown — harmless default for the dropdown
}

/** Which power profile the current args correspond to (engine + idle interval signature). */
function activeProfile(args: string[]): "saver" | "balanced" | "performance" | null {
  const ei = args.indexOf("--audio-transcription-engine");
  const engine = ei >= 0 ? args[ei + 1] : "";
  const hasIdle = args.includes("--idle-capture-interval-ms");
  const idleVal = hasIdle ? args[args.indexOf("--idle-capture-interval-ms") + 1] : "";
  if (engine === "whisper-tiny-quantized" && idleVal === "120000") return "saver";
  if (engine === "whisper-large-v3-turbo-quantized" && idleVal === "60000") return "balanced";
  if (engine === "whisper-large-v3-turbo" && !hasIdle) return "performance";
  return null;
}

function stat(label: string, val: string): string {
  return `<div class="stat"><span class="stat-label">${label}</span><span class="stat-val">${val}</span></div>`;
}

/** Just the live numbers + "actually capturing" line — cheap, safe to poll (no list rebuilds). */
export async function refreshPerfStats() {
  const [s, h, args] = await Promise.all([
    api.perfStats(),
    api.getHealth().catch(() => null),
    api.getRecordArgs().catch(() => [] as string[]),
  ]);
  const p = h?.pipeline ?? {};
  const ap = h?.audio_pipeline ?? {};
  const queued = ap.pending_transcription_segments ?? 0;
  $("perf-stats").innerHTML =
    stat("Recorder CPU", `${(s.cpu ?? 0).toFixed(1)}%`) +
    stat("Recorder RAM", `${s.rss_mb ?? 0} MB`) +
    stat("Audio queued", `${queued}${queued > 0 ? " ⏳" : " ✓"}`) +
    stat("Transcribed", String(ap.transcriptions_completed ?? 0)) +
    stat("DB size", `${s.db_mb ?? 0} MB`) +
    stat("Media size", `${s.data_mb ?? 0} MB`) +
    stat("Capture FPS", (p.capture_fps_actual ?? 0).toFixed(3));
  const videoOn = !args.includes("--disable-vision");
  const audioDisabled = (h?.audio_status ?? "") === "disabled";
  const frameOk = (h?.frame_status ?? "") === "ok";
  const audioSelected = !args.includes("--disable-audio") && args.includes("--audio-device");
  const screenState = !videoOn ? "Screen off" : frameOk ? "Screen ✓" : "Screen ⚠ stalled";
  const audioStateLabel = audioDisabled ? "Audio ✗ not capturing" : "Audio ✓";
  let status = `<b>Actually capturing:</b> ${screenState} · ${audioStateLabel}`;
  if (audioSelected && audioDisabled) {
    status +=
      " — a device is selected but the recorder isn't recording audio. The OS is likely blocking microphone access for the recorder (system privacy settings → Microphone → allow Meye Recorder).";
  }
  $("cap-status").innerHTML = status;
}

export async function refreshPerf() {
  await refreshPerfStats();
  const [args, devs] = await Promise.all([
    api.getRecordArgs().catch(() => [] as string[]),
    api.audioDevices().catch(() => null),
  ]);
  $("perf-flags").textContent = args.length ? args.join(" ") : "(defaults — no tuning applied)";
  const prof = activeProfile(args);
  for (const key of ["saver", "balanced", "performance"] as const) {
    ($(`perf-${key}`) as HTMLButtonElement).classList.toggle("primary", key === prof);
  }
  ($("perf-audio") as HTMLSelectElement).value = currentAudioKey(args);
  api
    .getDiscreet()
    .then((on) => (($("perf-discreet") as HTMLInputElement).checked = on))
    .catch(() => {});
  // Screen toggle reflects config; audio is a per-device picker built from /audio/list.
  ($("cap-video") as HTMLInputElement).checked = !args.includes("--disable-vision");
  renderAudioDevices(devs, args);
  // (perf-stats + the "actually capturing" line are handled by refreshPerfStats, called above)
}

async function applyArgs(args: string[]) {
  await wrap("Apply (restarting recorder)", () => api.setRecordArgs(args));
  setTimeout(() => void refreshPerf(), 3000);
}

/** Pause/resume screen (vision) capture by toggling --disable-vision. Exported so voice
 * commands can drive it too. Audio stays on so voice control keeps working while paused. */
export async function setVisionPaused(paused: boolean) {
  const args = await api.getRecordArgs().catch(() => [] as string[]);
  const has = args.includes("--disable-vision");
  if (paused === has) return; // already in the desired state
  const next = paused ? [...args, "--disable-vision"] : args.filter((a) => a !== "--disable-vision");
  await applyArgs(next);
}

/** Apply a power profile (model + cadence) while PRESERVING the user's capture-source choices
 * (which screens/mics are on), so switching profiles never silently re-enables the microphone. */
async function applyProfile(preset: string[]) {
  const cur = await api.getRecordArgs().catch(() => [] as string[]);
  const keepVision = cur.includes("--disable-vision") ? ["--disable-vision"] : [];
  if (cur.includes("--disable-audio")) {
    // Audio fully off — drop the preset's transcription engine and disable audio.
    const noEngine: string[] = [];
    for (let i = 0; i < preset.length; i++) {
      if (preset[i] === "--audio-transcription-engine") {
        i++;
        continue;
      }
      noEngine.push(preset[i]);
    }
    return void applyArgs([...noEngine, ...keepVision, "--disable-audio"]);
  }
  // Preserve any explicit --audio-device selection (e.g. mic off / system-audio only).
  const devs: string[] = [];
  for (let i = 0; i < cur.length; i++) if (cur[i] === "--audio-device") devs.push(cur[i + 1] ?? "");
  const keepDevices = devs.flatMap((d) => ["--audio-device", d]);
  void applyArgs([...preset, ...keepVision, ...keepDevices]);
}

/** Swap just the transcription engine, keeping the other flags. */
async function applyAudio(key: string) {
  const current = await api.getRecordArgs().catch(() => [] as string[]);
  void applyArgs([...stripAudio(current), ...(AUDIO[key] ?? [])]);
}

export function initPerformance() {
  $("perf-saver").onclick = () => void applyProfile(PRESETS.saver);
  $("perf-balanced").onclick = () => void applyProfile(PRESETS.balanced);
  $("perf-performance").onclick = () => void applyProfile(PRESETS.performance);
  ($("perf-audio") as HTMLSelectElement).onchange = (e) => void applyAudio((e.target as HTMLSelectElement).value);
  ($("perf-discreet") as HTMLInputElement).onchange = (e) => {
    const on = (e.target as HTMLInputElement).checked;
    void wrap(on ? "Discreet mode on" : "Discreet mode off", () => api.setDiscreet(on));
  };
  // Screen toggle; audio device toggles are wired per-row in renderAudioDevices().
  ($("cap-video") as HTMLInputElement).onchange = async (e) => {
    const cur = await api.getRecordArgs().catch(() => [] as string[]);
    void applyArgs(setVideoArg(cur, (e.target as HTMLInputElement).checked));
  };
}
