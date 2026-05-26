import { listen } from "@tauri-apps/api/event";
import { initTabs, goTab, type Tab } from "./tabs";
import { initHome, loadHome } from "./home";
import { initStatus, refreshHealth } from "./status";
import { initSearch } from "./search";
import { initTimeline, loadTimelineIfEmpty } from "./timeline";
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
initDevices();
initPipes();
initSettings();
initPerformance();
initVoice();

$("a-go").onclick = () =>
  wrap("SQL", async () => {
    $("a-out").textContent = JSON.stringify(await api.rawSql(($("a-sql") as HTMLTextAreaElement).value), null, 2);
  });

initTabs((tab: Tab) => {
  if (tab === "home") void loadHome();
  if (tab === "status") refreshHealth();
  if (tab === "timeline") loadTimelineIfEmpty();
  if (tab === "devices") void refreshDevices();
  if (tab === "pipes") void refreshPipes();
  if (tab === "settings") void refreshSettings();
  if (tab === "performance") void refreshPerf();
});

// Sidebar footer (status dot + label) is a shortcut to the Status tab — start/stop from there.
$("side-foot").onclick = () => goTab("status");

listen("status", (e) => (window as any).__meyeApplyStatus?.(e.payload));

refreshHealth();
setInterval(refreshHealth, 5000);
