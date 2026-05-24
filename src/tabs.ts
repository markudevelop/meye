import { $ } from "./ui";

const TABS = ["status", "search", "devices", "pipes", "advanced"] as const;
export type Tab = (typeof TABS)[number];

export function initTabs(onShow: (tab: Tab) => void) {
  for (const tab of TABS) {
    $(`tab-btn-${tab}`).onclick = () => select(tab, onShow);
  }
  select("status", onShow);
}

function select(tab: Tab, onShow: (tab: Tab) => void) {
  for (const t of TABS) {
    $(`tab-btn-${t}`).classList.toggle("active", t === tab);
    $(`panel-${t}`).classList.toggle("hidden", t !== tab);
  }
  onShow(tab);
}
