import { $, wrap } from "./ui";
import { api } from "./api";

type Status = "Healthy" | "Degraded" | "Down" | "NotInstalled" | "WaitingPermissions";

const PANE: Record<string, string> = {
  "Screen Recording": "screen",
  Microphone: "microphone",
  Accessibility: "accessibility",
};

function applyStatus(s: Status) {
  const cls = ({ Healthy: "green", Degraded: "yellow", Down: "red", NotInstalled: "grey", WaitingPermissions: "orange" } as Record<Status, string>)[s] ?? "grey";
  const label = ({
    Healthy: "Recording",
    Degraded: "Degraded",
    Down: "Stopped",
    NotInstalled: "Not set up",
    WaitingPermissions: "Waiting for permissions",
  } as Record<Status, string>)[s] ?? s;
  for (const id of ["dot", "side-dot"]) {
    const el = document.getElementById(id);
    if (el) el.className = "dot " + cls;
  }
  for (const id of ["status-text", "side-status"]) {
    const el = document.getElementById(id);
    if (el) el.textContent = label;
  }
  $("setup").classList.toggle("hidden", s !== "NotInstalled");
  $("controls").classList.toggle("hidden", s === "NotInstalled");
  const showPerms = s === "WaitingPermissions";
  $("perms").classList.toggle("hidden", !showPerms);
  if (showPerms) void renderPerms();

  // Reflect the live state in the buttons: highlight the action you'd actually take, and
  // disable the ones that don't apply (can't Start when already recording, etc).
  const running = s === "Healthy" || s === "Degraded";
  const stopped = s === "Down";
  const start = $("btn-start") as HTMLButtonElement;
  const stop = $("btn-stop") as HTMLButtonElement;
  const restart = $("btn-restart") as HTMLButtonElement;
  start.disabled = running;
  stop.disabled = !running;
  restart.disabled = !running;
  start.classList.toggle("primary", !running); // primary (highlighted) when stopped/idle
  stop.classList.remove("primary");

  // A plain-language hint when stopped (no /health to drive the reason line otherwise).
  const reason = $("health-reason");
  if (stopped) {
    reason.textContent = "⏹ Recording is stopped. Press Start to begin capturing again.";
    reason.classList.remove("hidden");
  }
}

async function renderPerms() {
  const p = await api.getPermissions();
  $("perms-msg").textContent = p.waiting.length
    ? `Waiting for permissions: ${p.waiting.join(", ")}. Grant in System Settings, then Re-check.`
    : "Starting…";
  const box = $("perms-buttons");
  box.innerHTML = "";
  for (const name of p.waiting) {
    const b = document.createElement("button");
    b.textContent = `Open ${name}`;
    b.onclick = () => wrap(`Open ${name}`, () => api.openSettings(PANE[name] ?? ""));
    box.appendChild(b);
  }
}

async function refreshState() {
  const st = await api.getState();
  if (!st.installed) applyStatus("NotInstalled");
  else {
    $("setup").classList.add("hidden");
    $("controls").classList.remove("hidden");
  }
}

export async function refreshHealth() {
  const h = await api.getHealth();
  if (!h) return;
  $("m-version").textContent = h.version || "—";
  const secs = Math.round(h.pipeline?.uptime_secs ?? 0);
  const d = Math.floor(secs / 86400);
  const hh = Math.floor((secs % 86400) / 3600);
  const mm = Math.floor((secs % 3600) / 60);
  $("m-uptime").textContent = secs <= 0 ? "—" : d ? `${d}d ${hh}h` : hh ? `${hh}h ${mm}m` : `${mm}m`;
  $("m-fps").textContent = (h.pipeline?.capture_fps_actual ?? 0).toFixed(3);
  $("m-audio").textContent = h.audio_status || "—";
  const pending = h.audio_pipeline?.pending_transcription_segments ?? 0;
  $("m-pending").textContent = String(pending);
  $("m-monitors").textContent = (h.monitors ?? []).join(", ") || "—";

  // Explain a non-healthy screenpipe status in plain language. A "degraded" caused only by a
  // transcription backlog (frame+audio ok) is benign — capture is still running.
  const reason = $("health-reason");
  const coreOk = h.frame_status === "ok" && h.audio_status === "ok";
  if (h.status && h.status !== "healthy" && h.status !== "ok") {
    if (coreOk && pending > 0) {
      reason.textContent = `✓ Recording normally — whisper is transcribing ${pending} audio segment${pending === 1 ? "" : "s"} in the background. This clears itself.`;
    } else {
      reason.textContent = h.message || "Some subsystems are not healthy — see logs below.";
    }
    reason.classList.remove("hidden");
  } else {
    reason.classList.add("hidden");
  }

  $("logs-pre").textContent = await api.tailLogs(40);
}

export function initStatus() {
  $("btn-setup").onclick = () => wrap("Setup", () => api.setup()).then(refreshState);
  $("btn-start").onclick = () => wrap("Start", () => api.start());
  $("btn-stop").onclick = () => wrap("Stop", () => api.stop());
  $("btn-restart").onclick = () => wrap("Restart", () => api.restart());
  $("btn-data").onclick = () => wrap("Open data", () => api.openData());
  $("btn-logs").onclick = () => wrap("Open logs", () => api.openLogs());
  $("btn-update").onclick = () => wrap("Update", () => api.update());
  $("btn-recheck").onclick = () => wrap("Re-check", () => api.recheck());
  refreshState();
  refreshHealth();
  // Exposed so main.ts's status-event listener can drive the dot without a circular import.
  (window as any).__meyeApplyStatus = applyStatus;
}
