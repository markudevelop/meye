import { $, wrap } from "./ui";
import { api } from "./api";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

/** Pull an array out of whatever shape the endpoint returns. */
function toList(res: any): any[] {
  if (Array.isArray(res)) return res;
  return res?.data ?? res?.devices ?? res?.monitors ?? [];
}

/** Render a device list as readable rows, with the raw JSON tucked into a <details>. */
function render(el: HTMLElement, res: any, label: string) {
  const items = toList(res);
  if (!items.length) {
    el.innerHTML = `<p class="meta">No ${label} found.</p>`;
    return;
  }
  const rows = items
    .map((it) => {
      if (it == null || typeof it !== "object") return `<div class="hit"><b>${esc(String(it))}</b></div>`;
      const name = it.name ?? it.device_name ?? it.id ?? "(unnamed)";
      const isDefault = it.is_default ?? it.default ?? false;
      const dims = it.width && it.height ? `${it.width}×${it.height}` : "";
      const extra = [dims, it.device_type, it.id != null && it.id !== name ? `id ${it.id}` : ""]
        .filter(Boolean)
        .join(" · ");
      return `<div class="hit"><b>${esc(String(name))}</b>${isDefault ? " <span class='meta'>(default)</span>" : ""}${
        extra ? ` <span class="meta">${esc(extra)}</span>` : ""
      }</div>`;
    })
    .join("");
  el.innerHTML = rows + `<details class="raw dev-only"><summary>Raw JSON</summary><pre>${esc(JSON.stringify(res, null, 2))}</pre></details>`;
}

export async function refreshDevices() {
  $("d-monitors").innerHTML = "<p class='meta'>Loading…</p>";
  $("d-audio").innerHTML = "<p class='meta'>Loading…</p>";
  try {
    render($("d-monitors"), await api.monitors(), "monitors");
  } catch (e) {
    $("d-monitors").innerHTML = `<p class="warn">Failed: ${esc(String(e))} (is the recorder running? see Status)</p>`;
  }
  try {
    render($("d-audio"), await api.audioDevices(), "audio devices");
  } catch (e) {
    $("d-audio").innerHTML = `<p class="warn">Failed: ${esc(String(e))} (is the recorder running? see Status)</p>`;
  }
}

export function initDevices() {
  $("d-refresh").onclick = () => void refreshDevices();
  $("d-audio-start").onclick = () => wrap("Start audio", () => api.audioStart()).then(refreshDevices);
  $("d-audio-stop").onclick = () => wrap("Stop audio", () => api.audioStop()).then(refreshDevices);
}
