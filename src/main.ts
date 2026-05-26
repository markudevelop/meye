import { listen } from "@tauri-apps/api/event";
import { initTabs, goTab, type Tab } from "./tabs";
import { initHome, loadHome, startNewChat } from "./home";
import { initStatus, refreshHealth } from "./status";
import { initMemory, startMemory, stopMemory } from "./memory";
import { initDevices, refreshDevices } from "./devices";
import { initPipes, refreshPipes } from "./pipes";
import { initSettings, refreshSettings } from "./settings";
import { initPerformance, refreshPerf, refreshPerfStats } from "./performance";
import { initPalette } from "./palette";
import { initVoice } from "./voice";
import { $, wrap } from "./ui";
import { api } from "./api";

// Restore developer-mode visibility before anything renders.
if (localStorage.getItem("meye.dev") === "1") document.body.classList.add("dev");

initPalette();
initHome();
initStatus();
initMemory();
initDevices();
initPipes();
initSettings();
initPerformance();
initVoice();

$("a-go").onclick = () =>
  wrap("SQL", async () => {
    $("a-out").textContent = JSON.stringify(await api.rawSql(($("a-sql") as HTMLTextAreaElement).value), null, 2);
  });

// Settings is one tab with sub-tabs (Recorder / Capture / Devices / AI); each sub refreshes
// the content that used to be its own tab.
const SUB_REFRESH: Record<string, () => void> = {
  recorder: () => refreshHealth(),
  capture: () => void refreshPerf(),
  devices: () => void refreshDevices(),
  ai: () => void refreshSettings(),
};
let activeSub = "recorder";

// One auto-refresh poller for whatever view is visible — no manual Refresh buttons. The global
// refreshHealth interval (below) keeps the Recorder sub current; this handles the live ones.
let poll: ReturnType<typeof setInterval> | null = null;
function poller(fn: (() => void) | null, ms = 4000) {
  if (poll) {
    clearInterval(poll);
    poll = null;
  }
  if (fn) {
    fn();
    poll = setInterval(fn, ms);
  }
}

export function showSettingsSub(name: string) {
  if (!SUB_REFRESH[name]) name = "recorder";
  activeSub = name;
  for (const s of Object.keys(SUB_REFRESH)) $(`sub-${s}`).classList.toggle("hidden", s !== name);
  document.querySelectorAll<HTMLElement>(".subtab").forEach((b) => b.classList.toggle("active", b.dataset.sub === name));
  // Capture: full render once (profiles, device toggles), then poll only the live stats so we
  // don't rebuild the interactive controls every few seconds.
  if (name === "capture") {
    void refreshPerf();
    poller(() => void refreshPerfStats(), 3000);
  } else {
    poller(null);
    SUB_REFRESH[name]();
  }
}
document.querySelectorAll<HTMLElement>(".subtab").forEach((b) => {
  b.onclick = () => showSettingsSub(b.dataset.sub || "recorder");
});

initTabs((tab: Tab) => {
  if (tab !== "memory") stopMemory();
  if (tab === "home") {
    poller(null);
    void loadHome();
  } else if (tab === "memory") {
    poller(null);
    startMemory();
  } else if (tab === "pipes") {
    poller(() => void refreshPipes(), 5000); // auto-refresh pipe status/last-run
  } else if (tab === "settings") {
    showSettingsSub(activeSub);
  } else {
    poller(null);
  }
});

// Clicking "Chat" in the sidebar starts a fresh new chat (not the previous conversation).
$("tab-btn-home").onclick = () => startNewChat();

// Sidebar footer (status dot + label) jumps to Settings → Recorder to start/stop.
$("side-foot").onclick = () => {
  goTab("settings");
  showSettingsSub("recorder");
};

listen("status", (e) => (window as any).__meyeApplyStatus?.(e.payload));

refreshHealth();
setInterval(refreshHealth, 5000);
