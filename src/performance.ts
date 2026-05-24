import { $, wrap } from "./ui";
import { api } from "./api";

const PRESETS: Record<string, string[]> = {
  saver: [
    "--audio-transcription-engine",
    "whisper-tiny-quantized",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "120000",
  ],
  balanced: [
    "--audio-transcription-engine",
    "whisper-large-v3-turbo-quantized",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "60000",
  ],
  performance: ["--audio-transcription-engine", "whisper-large-v3-turbo"],
};

const AUDIO: Record<string, string[]> = {
  off: ["--disable-audio"],
  tiny: ["--audio-transcription-engine", "whisper-tiny-quantized"],
  balanced: ["--audio-transcription-engine", "whisper-large-v3-turbo-quantized"],
  best: ["--audio-transcription-engine", "whisper-large-v3-turbo"],
};

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
  if (args.includes("--disable-audio")) return "off";
  const i = args.indexOf("--audio-transcription-engine");
  const v = i >= 0 ? args[i + 1] : "";
  if (v === "whisper-tiny-quantized") return "tiny";
  if (v === "whisper-large-v3-turbo") return "best";
  return "balanced";
}

function stat(label: string, val: string): string {
  return `<div class="stat"><span class="stat-label">${label}</span><span class="stat-val">${val}</span></div>`;
}

export async function refreshPerf() {
  const [s, h, args] = await Promise.all([
    api.perfStats(),
    api.getHealth().catch(() => null),
    api.getRecordArgs().catch(() => [] as string[]),
  ]);
  const p = h?.pipeline ?? {};
  $("perf-stats").innerHTML =
    stat("Recorder CPU", `${(s.cpu ?? 0).toFixed(1)}%`) +
    stat("Recorder RAM", `${s.rss_mb ?? 0} MB`) +
    stat("Capture FPS", (p.capture_fps_actual ?? 0).toFixed(3)) +
    stat("DB latency", `${Math.round(p.avg_db_latency_ms ?? 0)} ms`) +
    stat("DB size", `${s.db_mb ?? 0} MB`) +
    stat("Media size", `${s.data_mb ?? 0} MB`) +
    stat("Pending transcripts", String(h?.audio_pipeline?.pending_transcription_segments ?? 0));
  $("perf-flags").textContent = args.length ? args.join(" ") : "(defaults — no tuning applied)";
  ($("perf-audio") as HTMLSelectElement).value = currentAudioKey(args);
  ($("perf-pause") as HTMLButtonElement).textContent = args.includes("--disable-vision")
    ? "▶ Resume screen capture"
    : "⏸ Pause screen capture";
}

async function applyArgs(args: string[]) {
  await wrap("Apply (restarting recorder)", () => api.setRecordArgs(args));
  setTimeout(() => void refreshPerf(), 3000);
}

/** Swap just the transcription engine, keeping the other flags. */
async function applyAudio(key: string) {
  const current = await api.getRecordArgs().catch(() => [] as string[]);
  void applyArgs([...stripAudio(current), ...(AUDIO[key] ?? [])]);
}

export function initPerformance() {
  $("perf-refresh").onclick = () => void refreshPerf();
  $("perf-saver").onclick = () => void applyArgs(PRESETS.saver);
  $("perf-balanced").onclick = () => void applyArgs(PRESETS.balanced);
  $("perf-performance").onclick = () => void applyArgs(PRESETS.performance);
  ($("perf-audio") as HTMLSelectElement).onchange = (e) => void applyAudio((e.target as HTMLSelectElement).value);
  $("perf-pause").onclick = async () => {
    const args = await api.getRecordArgs();
    const has = args.includes("--disable-vision");
    const next = has ? args.filter((a) => a !== "--disable-vision") : [...args, "--disable-vision"];
    void applyArgs(next);
  };
}
