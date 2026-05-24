import { $ } from "./ui";

const TABS = ["status", "search", "chat", "timeline", "devices", "pipes", "performance", "settings", "advanced"] as const;
export type Tab = (typeof TABS)[number];

let onShowCb: (tab: Tab) => void = () => {};

export function initTabs(onShow: (tab: Tab) => void) {
  onShowCb = onShow;
  for (const tab of TABS) {
    $(`tab-btn-${tab}`).onclick = () => select(tab);
  }
  select("status");
}

/** Programmatically switch tabs (used by the command palette). */
export function goTab(tab: Tab) {
  select(tab);
}

function select(tab: Tab) {
  for (const t of TABS) {
    $(`tab-btn-${t}`).classList.toggle("active", t === tab);
    $(`panel-${t}`).classList.toggle("hidden", t !== tab);
  }
  onShowCb(tab);
}
