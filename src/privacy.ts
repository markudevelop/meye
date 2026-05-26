import { $, wrap } from "./ui";
import { api } from "./api";

// Privacy & Storage: shows disk usage, lets you auto-delete old recordings (retention), and
// permanently delete recent recordings. Destructive actions use a two-click confirm.

function stat(label: string, val: string): string {
  return `<div class="stat"><span class="stat-label">${label}</span><span class="stat-val">${val}</span></div>`;
}

export async function refreshPrivacy() {
  // Storage usage
  api
    .perfStats()
    .then((s) => {
      const total = (s.db_mb ?? 0) + (s.data_mb ?? 0);
      $("priv-storage").innerHTML =
        stat("Total on disk", `${total} MB`) + stat("Media (video/audio)", `${s.data_mb ?? 0} MB`) + stat("Index/database", `${s.db_mb ?? 0} MB`);
    })
    .catch(() => {});

  // Retention state
  try {
    const r = await api.retentionStatus();
    const days = r.enabled ? r.retention_days ?? 0 : 0;
    ($("priv-retention") as HTMLSelectElement).value = String(days);
    ($("priv-mode-all") as HTMLInputElement).checked = r.mode === "all";
    $("priv-retention-status").textContent = r.enabled
      ? `Auto-deleting recordings older than ${r.retention_days} days (${r.mode === "all" ? "everything" : "media only"}).${
          r.total_deleted ? ` Freed ${r.total_deleted} items so far.` : ""
        }`
      : "Off — recordings are kept forever.";
  } catch {
    $("priv-retention-status").textContent = "Couldn't read retention status (is the recorder running?).";
  }
}

async function applyRetention() {
  const days = Number(($("priv-retention") as HTMLSelectElement).value) || 0;
  const mode = ($("priv-mode-all") as HTMLInputElement).checked ? "all" : "media";
  await wrap(days ? `Keep last ${days} days` : "Keep forever", () => api.retentionConfigure(days > 0, days || 14, mode));
  void refreshPrivacy();
}

// Two-click confirm for destructive deletes.
let armed: string | null = null;
let armedTimer: ReturnType<typeof setTimeout> | null = null;
function confirmClick(btn: HTMLButtonElement, key: string, label: string, run: () => Promise<void>) {
  if (armed === key) {
    if (armedTimer) clearTimeout(armedTimer);
    armed = null;
    btn.textContent = label;
    btn.classList.remove("danger");
    void run();
    return;
  }
  armed = key;
  btn.textContent = "Click again to confirm";
  btn.classList.add("danger");
  if (armedTimer) clearTimeout(armedTimer);
  armedTimer = setTimeout(() => {
    armed = null;
    btn.textContent = label;
    btn.classList.remove("danger");
  }, 3500);
}

async function deleteSince(start: Date, what: string) {
  const startIso = start.toISOString();
  const endIso = new Date().toISOString();
  await wrap(`Delete ${what}`, async () => {
    await api.deleteRange(startIso, endIso);
  });
  $("priv-del-status").textContent = `Deleted ${what}.`;
  void refreshPrivacy();
}

export function initPrivacy() {
  ($("priv-retention") as HTMLSelectElement).onchange = () => void applyRetention();
  ($("priv-mode-all") as HTMLInputElement).onchange = () => void applyRetention();
  const hour = $("priv-del-hour") as HTMLButtonElement;
  hour.onclick = () =>
    confirmClick(hour, "hour", "Delete last hour", () => deleteSince(new Date(Date.now() - 3600_000), "the last hour"));
  const today = $("priv-del-today") as HTMLButtonElement;
  today.onclick = () =>
    confirmClick(today, "today", "Delete today", () => {
      const d = new Date();
      d.setHours(0, 0, 0, 0);
      return deleteSince(d, "today's recordings");
    });
}
