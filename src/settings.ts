import { $, wrap } from "./ui";
import { api } from "./api";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

export async function refreshSettings() {
  const out = $("set-list");
  out.textContent = "Loading…";
  try {
    const res = await api.modelsList();
    const presets: any[] = Array.isArray(res) ? res : (res.data ?? []);
    if (!presets.length) {
      out.innerHTML = "<p class='meta'>No AI presets yet — add one below so pipes can run.</p>";
      return;
    }
    out.innerHTML = "";
    for (const p of presets) {
      const id: string = p.id ?? p.name ?? "?";
      const provider: string = p.provider ?? "";
      const model: string = p.model ?? "";
      const isDefault: boolean = p.default ?? p.is_default ?? false;
      const card = document.createElement("div");
      card.className = "hit";
      card.innerHTML = `<b>${esc(id)}</b>${isDefault ? " <span class='meta'>(default)</span>" : ""} <span class="meta">${esc(provider)} · ${esc(model)}</span>`;
      const row = document.createElement("div");
      row.className = "row";
      if (!isDefault) {
        const d = document.createElement("button");
        d.textContent = "Set default";
        d.onclick = () => wrap(`Default ${id}`, () => api.modelsSetDefault(id)).then(refreshSettings);
        row.appendChild(d);
      }
      const del = document.createElement("button");
      del.textContent = "Delete";
      del.onclick = () => wrap(`Delete ${id}`, () => api.modelsDelete(id)).then(refreshSettings);
      row.appendChild(del);
      card.appendChild(row);
      out.appendChild(card);
    }
  } catch (e) {
    out.innerHTML = `<p class="warn">Failed to load presets: ${esc(String(e))}</p>`;
  }
}

function syncFields() {
  const provider = ($("set-provider") as HTMLSelectElement).value;
  const needsUrl = provider === "native-ollama" || provider === "custom";
  const needsKey = provider === "openai" || provider === "anthropic" || provider === "custom";
  $("set-url-wrap").classList.toggle("hidden", !needsUrl);
  $("set-key-wrap").classList.toggle("hidden", !needsKey);
}

export function initSettings() {
  ($("set-provider") as HTMLSelectElement).onchange = syncFields;
  syncFields();
  $("set-create").onclick = () =>
    wrap("Create preset", () =>
      api.modelsCreate({
        id: ($("set-id") as HTMLInputElement).value.trim(),
        provider: ($("set-provider") as HTMLSelectElement).value,
        model: ($("set-model") as HTMLInputElement).value.trim(),
        url: ($("set-url") as HTMLInputElement).value.trim() || undefined,
        apiKey: ($("set-key") as HTMLInputElement).value.trim() || undefined,
        setDefault: ($("set-default") as HTMLInputElement).checked,
      })
    ).then(() => {
      ($("set-key") as HTMLInputElement).value = "";
      refreshSettings();
    });
}
