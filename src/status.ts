import { $, wrap } from "./ui";
import { api } from "./api";

type Status = "Healthy" | "Degraded" | "Down" | "NotInstalled" | "WaitingPermissions";

const PANE: Record<string, string> = {
  "Screen Recording": "screen",
  Microphone: "microphone",
  Accessibility: "accessibility",
};

function applyStatus(s: Status) {
  const dot = $("dot");
  const cls = ({ Healthy: "green", Degraded: "yellow", Down: "red", NotInstalled: "grey", WaitingPermissions: "orange" } as Record<Status, string>)[s] ?? "grey";
  dot.className = "dot " + cls;
  $("status-label").textContent = s === "WaitingPermissions" ? "Waiting for permissions" : s;
  $("setup").classList.toggle("hidden", s !== "NotInstalled");
  $("controls").classList.toggle("hidden", s === "NotInstalled");
  const showPerms = s === "WaitingPermissions";
  $("perms").classList.toggle("hidden", !showPerms);
  if (showPerms) void renderPerms();
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
  $("m-uptime").textContent = `${Math.round(h.pipeline?.uptime_secs ?? 0)} s`;
  $("m-fps").textContent = (h.pipeline?.capture_fps_actual ?? 0).toFixed(3);
  $("m-audio").textContent = h.audio_status || "—";
  $("m-pending").textContent = String(h.audio_pipeline?.pending_transcription_segments ?? 0);
  $("m-monitors").textContent = (h.monitors ?? []).join(", ") || "—";
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
