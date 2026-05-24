import { $, wrap } from "./ui";
import { api } from "./api";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

let editing: string | null = null;

async function openEditor(name: string) {
  editing = name;
  $("p-editing").textContent = name;
  ($("p-config") as HTMLTextAreaElement).value = "Loading…";
  $("p-editor").classList.remove("hidden");
  try {
    ($("p-config") as HTMLTextAreaElement).value = await api.pipeConfigRead(name);
  } catch (e) {
    ($("p-config") as HTMLTextAreaElement).value = `# failed to read config: ${e}`;
  }
}

export async function refreshPipes() {
  const out = $("p-list");
  out.textContent = "Loading…";
  try {
    const res = await api.pipeList();
    const pipes: any[] = Array.isArray(res) ? res : (res.data ?? res.pipes ?? []);
    if (!pipes.length) {
      out.textContent = "No pipes installed.";
      return;
    }
    out.innerHTML = "";
    for (const p of pipes) {
      const cfg = p.config ?? p;
      const name: string = cfg.name ?? p.name ?? "?";
      const enabled: boolean = cfg.enabled ?? p.enabled ?? false;
      const schedule: string = cfg.schedule ?? p.schedule ?? "manual";
      const lastRun: string = p.last_run ?? cfg.last_run ?? "never";
      const running = p.is_running ? " · running" : "";

      const card = document.createElement("div");
      card.className = "hit";
      card.innerHTML = `<b>${esc(name)}</b> <span class="meta">${enabled ? "enabled" : "disabled"} · ${esc(schedule)} · last: ${esc(String(lastRun))}${running}</span>`;

      const controls = document.createElement("div");
      controls.className = "row";

      const runBtn = document.createElement("button");
      runBtn.textContent = "Run now";
      runBtn.onclick = () => wrap(`Run ${name}`, () => api.pipeRun(name)).then(refreshPipes);
      controls.appendChild(runBtn);

      const toggleBtn = document.createElement("button");
      toggleBtn.textContent = enabled ? "Disable" : "Enable";
      toggleBtn.onclick = () =>
        wrap(`${enabled ? "Disable" : "Enable"} ${name}`, () => (enabled ? api.pipeDisable(name) : api.pipeEnable(name))).then(refreshPipes);
      controls.appendChild(toggleBtn);

      const editBtn = document.createElement("button");
      editBtn.textContent = "Edit config";
      editBtn.onclick = () => void openEditor(name);
      controls.appendChild(editBtn);

      const logsBtn = document.createElement("button");
      logsBtn.textContent = "Logs";
      logsBtn.onclick = async () => {
        $("p-logs").textContent = "Loading…";
        try {
          $("p-logs").textContent = (await api.pipeLogs(name)) || "(no logs)";
        } catch (e) {
          $("p-logs").textContent = `Failed: ${e}`;
        }
      };
      controls.appendChild(logsBtn);

      const delBtn = document.createElement("button");
      delBtn.textContent = "Delete";
      delBtn.onclick = () => wrap(`Delete ${name}`, () => api.pipeDelete(name)).then(refreshPipes);
      controls.appendChild(delBtn);

      card.appendChild(controls);

      // Per-pipe AI preset assignment (space-separated ids = fallback chain).
      const presetRow = document.createElement("div");
      presetRow.className = "row";
      const presetInput = document.createElement("input");
      presetInput.placeholder = "preset id(s) for this pipe";
      const presetBtn = document.createElement("button");
      presetBtn.textContent = "Set preset";
      presetBtn.onclick = () => {
        const ids = presetInput.value.trim().split(/\s+/).filter(Boolean);
        if (!ids.length) return;
        void wrap(`Preset ${name}`, () => api.pipeSetPreset(name, ids));
      };
      presetRow.appendChild(presetInput);
      presetRow.appendChild(presetBtn);
      card.appendChild(presetRow);

      out.appendChild(card);
    }
  } catch (e) {
    out.innerHTML = `<p class="warn">Failed to load pipes: ${esc(String(e))} (is the recorder running? see Status tab)</p>`;
  }
}

async function renderRegistry(query: string) {
  if (!query) return;
  const out = $("r-results");
  out.textContent = "Searching registry…";
  try {
    const res = await api.registrySearch(query);
    const rows: any[] = Array.isArray(res) ? res : (res.data ?? []);
    if (!rows.length) {
      out.innerHTML = "<p class='meta'>No matches in the registry.</p>";
      return;
    }
    out.innerHTML = "";
    for (const r of rows) {
      const slug = String(r.slug ?? "");
      const card = document.createElement("div");
      card.className = "hit";
      card.innerHTML = `<b>${esc(slug)}</b> <span class="meta">${esc(String(r.category ?? ""))} · ${esc(String(r.installs ?? ""))} installs</span><div>${esc(String(r.description ?? ""))}</div>`;
      const row = document.createElement("div");
      row.className = "row";
      const inst = document.createElement("button");
      inst.textContent = "Install";
      inst.onclick = () => wrap(`Install ${slug}`, () => api.registryInstall(slug)).then(refreshPipes);
      row.appendChild(inst);
      card.appendChild(row);
      out.appendChild(card);
    }
  } catch (e) {
    out.innerHTML = `<p class="warn">Registry search failed: ${esc(String(e))}</p>`;
  }
}

export function initPipes() {
  $("p-refresh").onclick = () => void refreshPipes();
  $("r-go").onclick = () => void renderRegistry(($("r-q") as HTMLInputElement).value.trim());
  ($("r-q") as HTMLInputElement).addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") void renderRegistry(($("r-q") as HTMLInputElement).value.trim());
  });
  $("p-config-cancel").onclick = () => {
    editing = null;
    $("p-editor").classList.add("hidden");
  };
  $("p-config-save").onclick = () => {
    if (!editing) return;
    const name = editing;
    void wrap(`Save ${name}`, () => api.pipeConfigWrite(name, ($("p-config") as HTMLTextAreaElement).value)).then(() => {
      $("p-editor").classList.add("hidden");
      editing = null;
      refreshPipes();
    });
  };
}
