import { listen } from "@tauri-apps/api/event";
import { initTabs, goTab, type Tab } from "./tabs";
import { initHome, loadHome, startNewChat } from "./home";
import { initStatus, refreshHealth } from "./status";
import { initSearch } from "./search";
import { initTimeline, loadTimelineIfEmpty } from "./timeline";
import { initLive, startLive, stopLive } from "./live";
import { initDevices, refreshDevices } from "./devices";
import { initPipes, refreshPipes } from "./pipes";
import { initSettings, refreshSettings } from "./settings";
import { initPerformance, refreshPerf } from "./performance";
import { initPalette } from "./palette";
import { initVoice } from "./voice";
import { $, wrap } from "./ui";
import { api } from "./api";

// Restore developer-mode visibility before anything renders.
if (localStorage.getItem("meye.dev") === "1") document.body.classList.add("dev");

initPalette();
initHome();
initStatus();
initSearch();
initTimeline();
initLive();
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
export function showSettingsSub(name: string) {
  if (!SUB_REFRESH[name]) name = "recorder";
  activeSub = name;
  for (const s of Object.keys(SUB_REFRESH)) $(`sub-${s}`).classList.toggle("hidden", s !== name);
  document.querySelectorAll<HTMLElement>(".subtab").forEach((b) => b.classList.toggle("active", b.dataset.sub === name));
  SUB_REFRESH[name]();
}
document.querySelectorAll<HTMLElement>(".subtab").forEach((b) => {
  b.onclick = () => showSettingsSub(b.dataset.sub || "recorder");
});

initTabs((tab: Tab) => {
  if (tab === "home") void loadHome();
  if (tab === "timeline") loadTimelineIfEmpty();
  if (tab === "live") startLive();
  else stopLive();
  if (tab === "pipes") void refreshPipes();
  if (tab === "settings") showSettingsSub(activeSub);
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
