import { listen } from "@tauri-apps/api/event";
import { initTabs, type Tab } from "./tabs";
import { initStatus, refreshHealth } from "./status";
import { initSearch } from "./search";
import { initChat } from "./chat";
import { initTimeline, loadTimelineIfEmpty } from "./timeline";
import { initDevices } from "./devices";
import { initPipes, refreshPipes } from "./pipes";
import { initSettings, refreshSettings } from "./settings";
import { initPerformance, refreshPerf } from "./performance";
import { initPalette } from "./palette";
import { $, wrap } from "./ui";
import { api } from "./api";

initPalette();
initStatus();
initSearch();
initChat();
initTimeline();
initDevices();
initPipes();
initSettings();
initPerformance();

$("a-go").onclick = () =>
  wrap("SQL", async () => {
    $("a-out").textContent = JSON.stringify(await api.rawSql(($("a-sql") as HTMLTextAreaElement).value), null, 2);
  });

initTabs((tab: Tab) => {
  if (tab === "status") refreshHealth();
  if (tab === "timeline") loadTimelineIfEmpty();
  if (tab === "pipes") void refreshPipes();
  if (tab === "settings") void refreshSettings();
  if (tab === "performance") void refreshPerf();
});

listen("status", (e) => (window as any).__meyeApplyStatus?.(e.payload));

refreshHealth();
setInterval(refreshHealth, 5000);
