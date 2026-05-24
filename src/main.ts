import { listen } from "@tauri-apps/api/event";
import { initTabs, type Tab } from "./tabs";
import { initStatus, refreshHealth } from "./status";
import { initSearch } from "./search";
import { initDevices } from "./devices";
import { initPipes, refreshPipes } from "./pipes";
import { $, wrap } from "./ui";
import { api } from "./api";

initStatus();
initSearch();
initDevices();
initPipes();

$("a-go").onclick = () =>
  wrap("SQL", async () => {
    $("a-out").textContent = JSON.stringify(await api.rawSql(($("a-sql") as HTMLTextAreaElement).value), null, 2);
  });

initTabs((tab: Tab) => {
  if (tab === "status") refreshHealth();
  if (tab === "pipes") void refreshPipes();
});

listen("status", (e) => (window as any).__meyeApplyStatus?.(e.payload));

refreshHealth();
setInterval(refreshHealth, 5000);
