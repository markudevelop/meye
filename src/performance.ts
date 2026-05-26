import { $, wrap, toast } from "./ui";
import { api } from "./api";
import { isVoiceEnabled, setVoiceEnabled } from "./voice";

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

// Live audio device names (filled by refreshPerf from /audio/list) used to build
// --audio-device flags for the Microphone / Computer-audio capture toggles.
let micName = "";
let sysName = "";

function pickDevices(res: any): { mic: string; sys: string } {
  const arr: any[] = Array.isArray(res) ? res : (res?.data ?? []);
  const inputs = arr.filter((d) => /\(input\)/i.test(d?.name ?? ""));
  const outputs = arr.filter((d) => /\(output\)|system audio/i.test(d?.name ?? ""));
  const mic = (inputs.find((d) => d.is_default) ?? inputs[0])?.name ?? "";
  const sys = (outputs.find((d) => d.is_default) ?? outputs[0])?.name ?? "";
  return { mic, sys };
}

/** Audio devices currently selected in args. No --audio-device + no --disable-audio = defaults (both on). */
function audioState(args: string[]): { mic: boolean; pc: boolean } {
  if (args.includes("--disable-audio")) return { mic: false, pc: false };
  const devs: string[] = [];
  for (let i = 0; i < args.length; i++) if (args[i] === "--audio-device") devs.push(args[i + 1]);
  if (!devs.length) return { mic: true, pc: true };
  return { mic: micName !== "" && devs.includes(micName), pc: sysName !== "" && devs.includes(sysName) };
}

/** Strip all audio source/engine flags so we can rebuild them cleanly. */
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

/** Rebuild args for a given mic/pc selection, preserving the transcription engine (adding a default if missing). */
function buildAudioArgs(args: string[], mic: boolean, pc: boolean): string[] {
  let out = stripAudioSources(args);
  if (!mic && !pc) {
    out = out.filter((a, i) => a !== "--audio-transcription-engine" && out[i - 1] !== "--audio-transcription-engine");
    out.push("--disable-audio");
    return out;
  }
  if (!out.includes("--audio-transcription-engine")) out.push("--audio-transcription-engine", "whisper-tiny-quantized");
  if (mic && micName) out.push("--audio-device", micName);
  if (pc && sysName) out.push("--audio-device", sysName);
  return out;
}

function setVideoArg(args: string[], on: boolean): string[] {
  const without = args.filter((a) => a !== "--disable-vision");
  return on ? without : [...without, "--disable-vision"];
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

export async function refreshPerf() {
  const [s, h, args, devs] = await Promise.all([
    api.perfStats(),
    api.getHealth().catch(() => null),
    api.getRecordArgs().catch(() => [] as string[]),
    api.audioDevices().catch(() => null),
  ]);
  if (devs) ({ mic: micName, sys: sysName } = pickDevices(devs));
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
  ($("perf-voice") as HTMLInputElement).checked = isVoiceEnabled();
  // Capture-source toggles reflect the configured intent (the args)...
  const aud = audioState(args);
  const videoOn = !args.includes("--disable-vision");
  ($("cap-video") as HTMLInputElement).checked = videoOn;
  ($("cap-mic") as HTMLInputElement).checked = aud.mic;
  ($("cap-pc") as HTMLInputElement).checked = aud.pc;

  // ...but show what the recorder is ACTUALLY capturing (from /health), since the two can
  // disagree — e.g. audio "enabled" in settings but not actually capturing.
  const audioDisabled = (h?.audio_status ?? "") === "disabled";
  const frameOk = (h?.frame_status ?? "") === "ok";
  const screenState = !videoOn ? "Screen off" : frameOk ? "Screen ✓" : "Screen ⚠ stalled";
  const audioState_ = audioDisabled ? "Audio ✗ not capturing" : "Audio ✓";
  let status = `<b>Actually capturing:</b> ${screenState} · ${audioState_}`;
  if ((aud.mic || aud.pc) && audioDisabled) {
    status +=
      " — audio is turned on here but the recorder isn't recording any. Computer-audio-only doesn't start capture on macOS; enable Microphone for audio, or this stays screen-only.";
  }
  $("cap-status").innerHTML = status;
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
  $("perf-refresh").onclick = () => void refreshPerf();
  $("perf-saver").onclick = () => void applyProfile(PRESETS.saver);
  $("perf-balanced").onclick = () => void applyProfile(PRESETS.balanced);
  $("perf-performance").onclick = () => void applyProfile(PRESETS.performance);
  ($("perf-audio") as HTMLSelectElement).onchange = (e) => void applyAudio((e.target as HTMLSelectElement).value);
  ($("perf-discreet") as HTMLInputElement).onchange = (e) => {
    const on = (e.target as HTMLInputElement).checked;
    void wrap(on ? "Discreet mode on" : "Discreet mode off", () => api.setDiscreet(on));
  };
  ($("perf-voice") as HTMLInputElement).onchange = (e) => {
    const on = (e.target as HTMLInputElement).checked;
    setVoiceEnabled(on);
    toast(on ? "🎙 Voice button shown — tap it, then speak a command" : "Voice button hidden");
  };

  // Capture-source toggles (Screen / Microphone / Computer audio).
  const onCapture = async (which: "video" | "mic" | "pc", on: boolean) => {
    const cur = await api.getRecordArgs().catch(() => [] as string[]);
    if (which === "video") return void applyArgs(setVideoArg(cur, on));
    const st = audioState(cur);
    return void applyArgs(buildAudioArgs(cur, which === "mic" ? on : st.mic, which === "pc" ? on : st.pc));
  };
  ($("cap-video") as HTMLInputElement).onchange = (e) => void onCapture("video", (e.target as HTMLInputElement).checked);
  ($("cap-mic") as HTMLInputElement).onchange = (e) => void onCapture("mic", (e.target as HTMLInputElement).checked);
  ($("cap-pc") as HTMLInputElement).onchange = (e) => void onCapture("pc", (e.target as HTMLInputElement).checked);
}
