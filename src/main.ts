import { listen } from "@tauri-apps/api/event";
import { initTabs, type Tab } from "./tabs";
import { initHome, loadHome, startNewChat } from "./home";
import { initStatus, refreshHealth } from "./status";
import { initSearch } from "./search";
import { initTimeline, loadTimelineIfEmpty } from "./timeline";
import { initDevices } from "./devices";
import { initPipes, refreshPipes } from "./pipes";
import { initSchedule, refreshSchedule } from "./schedule";
import { initSettings, refreshSettings } from "./settings";
import { initPerformance, refreshPerf } from "./performance";
import { initPalette } from "./palette";
import { $, wrap } from "./ui";
import { api } from "./api";

initPalette();
initHome();
initStatus();
initSearch();
initTimeline();
initDevices();
initPipes();
initSchedule();
initSettings();
initPerformance();

$("a-go").onclick = () =>
  wrap("SQL", async () => {
    $("a-out").textContent = JSON.stringify(await api.rawSql(($("a-sql") as HTMLTextAreaElement).value), null, 2);
  });

initTabs((tab: Tab) => {
  if (tab === "home") void loadHome();
  if (tab === "status") refreshHealth();
  if (tab === "timeline") loadTimelineIfEmpty();
  if (tab === "pipes") void refreshPipes();
  if (tab === "schedule") void refreshSchedule();
  if (tab === "settings") void refreshSettings();
  if (tab === "performance") void refreshPerf();
});

// The "New Chat" nav item starts a fresh conversation (not just navigate).
$("tab-btn-home").onclick = () => startNewChat();

listen("status", (e) => (window as any).__meyeApplyStatus?.(e.payload));

refreshHealth();
setInterval(refreshHealth, 5000);
