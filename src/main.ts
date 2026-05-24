import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Status = "Healthy" | "Degraded" | "Down" | "NotInstalled";

const $ = (id: string) => document.getElementById(id)!;

function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  setTimeout(() => t.classList.add("hidden"), 4000);
}

async function wrap(label: string, fn: () => Promise<unknown>) {
  try {
    await fn();
    toast(`${label} ✓`);
  } catch (e) {
    toast(`${label} failed: ${e}`);
  }
  await refreshState();
}

function applyStatus(s: Status) {
  const dot = $("dot");
  dot.className =
    "dot " +
    ({ Healthy: "green", Degraded: "yellow", Down: "red", NotInstalled: "grey" }[s] ?? "grey");
  $("status-label").textContent = s;
  $("setup").classList.toggle("hidden", s !== "NotInstalled");
  $("controls").classList.toggle("hidden", s === "NotInstalled");
}

async function refreshState() {
  const st = (await invoke("get_state")) as { installed: boolean; pinned: boolean; loaded: boolean };
  if (!st.installed) {
    applyStatus("NotInstalled");
  } else {
    $("setup").classList.add("hidden");
    $("controls").classList.remove("hidden");
  }
}

async function refreshHealth() {
  const h = (await invoke("get_health")) as any | null;
  if (!h) return;
  $("m-version").textContent = h.version || "—";
  $("m-uptime").textContent = `${Math.round(h.pipeline?.uptime_secs ?? 0)} s`;
  $("m-fps").textContent = (h.pipeline?.capture_fps_actual ?? 0).toFixed(3);
  $("m-audio").textContent = h.audio_status || "—";
  $("m-pending").textContent = String(h.audio_pipeline?.pending_transcription_segments ?? 0);
  $("m-monitors").textContent = (h.monitors ?? []).join(", ") || "—";
  $("logs-pre").textContent = (await invoke("tail_logs", { lines: 40 })) as string;
}

function wire() {
  $("btn-setup").onclick = () => wrap("Setup", () => invoke("setup"));
  $("btn-start").onclick = () => wrap("Start", () => invoke("start"));
  $("btn-stop").onclick = () => wrap("Stop", () => invoke("stop"));
  $("btn-restart").onclick = () => wrap("Restart", () => invoke("restart"));
  $("btn-data").onclick = () => wrap("Open data", () => invoke("open_data_dir"));
  $("btn-logs").onclick = () => wrap("Open logs", () => invoke("open_logs"));
  $("btn-update").onclick = () => wrap("Update", () => invoke("update_screenpipe"));
}

listen<Status>("status", (e) => applyStatus(e.payload));

wire();
refreshState();
refreshHealth();
setInterval(refreshHealth, 5000);
