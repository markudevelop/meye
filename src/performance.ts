import { $, wrap } from "./ui";
import { api } from "./api";

const PRESETS: Record<string, string[]> = {
  saver: [
    "--audio-transcription-engine",
    "whisper-large-v3-turbo-quantized",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "120000",
  ],
  balanced: ["--disable-clipboard-capture", "--idle-capture-interval-ms", "60000"],
  performance: [],
};

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
  ($("perf-pause") as HTMLButtonElement).textContent = args.includes("--disable-vision")
    ? "▶ Resume screen capture"
    : "⏸ Pause screen capture";
}

async function applyArgs(args: string[]) {
  await wrap("Apply profile (restarting recorder)", () => api.setRecordArgs(args));
  setTimeout(() => void refreshPerf(), 3000);
}

export function initPerformance() {
  $("perf-refresh").onclick = () => void refreshPerf();
  $("perf-saver").onclick = () => void applyArgs(PRESETS.saver);
  $("perf-balanced").onclick = () => void applyArgs(PRESETS.balanced);
  $("perf-performance").onclick = () => void applyArgs(PRESETS.performance);
  $("perf-pause").onclick = async () => {
    const args = await api.getRecordArgs();
    const has = args.includes("--disable-vision");
    const next = has ? args.filter((a) => a !== "--disable-vision") : [...args, "--disable-vision"];
    void applyArgs(next);
  };
}
