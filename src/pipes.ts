import { $, wrap } from "./ui";
import { api } from "./api";
import { logAction } from "./home";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

// Preset value -> the exact schedule string written to pipe.md frontmatter.
// screenpipe accepts cron macros (@hourly/@daily/@weekly), "every Nm/Nh", and "manual".
const PRESETS: { key: string; label: string; schedule: string }[] = [
  { key: "manual", label: "Manual (off)", schedule: "manual" },
  { key: "30m", label: "Every 30 min", schedule: "every 30m" },
  { key: "hourly", label: "Hourly", schedule: "@hourly" },
  { key: "daily", label: "Daily", schedule: "@daily" },
  { key: "weekly", label: "Weekly", schedule: "@weekly" },
  { key: "custom", label: "Custom…", schedule: "" },
];

/** Map a frontmatter schedule string to a preset key (or "custom" if it's none of them). */
function scheduleKey(schedule: string): string {
  const s = (schedule ?? "").trim();
  const hit = PRESETS.find((p) => p.key !== "custom" && p.schedule === s);
  return hit ? hit.key : "custom";
}

async function applySchedule(name: string, schedule: string) {
  await wrap(`Schedule ${name} → ${schedule}`, () => api.pipeSetSchedule(name, schedule));
  void refreshPipes();
}

/** Build the "Runs: [dropdown] [custom cron]" control for a pipe card. */
function scheduleControl(name: string, schedule: string): HTMLElement {
  const key = scheduleKey(schedule);
  const row = document.createElement("div");
  row.className = "row sched-inline";

  const label = document.createElement("span");
  label.className = "meta";
  label.textContent = "Runs:";

  const sel = document.createElement("select");
  sel.className = "sched-select";
  sel.innerHTML = PRESETS.map(
    (p) => `<option value="${p.key}"${p.key === key ? " selected" : ""}>${p.label}</option>`
  ).join("");

  const custom = document.createElement("input");
  custom.className = "sched-custom" + (key === "custom" ? "" : " hidden");
  custom.placeholder = "cron or 'every 30m'";
  if (key === "custom") custom.value = schedule;

  sel.onchange = () => {
    if (sel.value === "custom") {
      custom.classList.remove("hidden");
      custom.focus();
      return; // wait for the user to type + commit
    }
    custom.classList.add("hidden");
    void applySchedule(name, PRESETS.find((x) => x.key === sel.value)!.schedule);
  };

  // Custom cron applies on Enter or blur — never per keystroke.
  const commit = () => {
    const v = custom.value.trim();
    if (v) void applySchedule(name, v);
  };
  custom.onkeydown = (e) => {
    if ((e as KeyboardEvent).key === "Enter") {
      e.preventDefault();
      commit();
    }
  };
  custom.onblur = commit;

  row.append(label, sel, custom);
  return row;
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
  // Auto-refresh runs on a timer — skip a rebuild while the user is interacting with the list
  // (e.g. typing a custom cron), so we never steal focus or wipe input.
  if (out.children.length && document.activeElement && out.contains(document.activeElement)) return;
  if (!out.children.length || out.textContent === "—") out.innerHTML = '<div class="loading-row"><span class="run-spin"></span> Loading…</div>';
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
      const title: string = cfg.title ?? name;
      const icon: string = cfg.icon ?? "⚙️";
      const desc: string = cfg.description ?? "";
      const enabled: boolean = cfg.enabled ?? p.enabled ?? false;
      const schedule: string = cfg.schedule ?? p.schedule ?? "manual";
      const lastRun: string = p.last_run ?? cfg.last_run ?? "never";
      const running = p.is_running ? " · running" : "";
      const lastError = p.last_error ?? cfg.last_error;
      const lastSuccess = p.last_success ?? cfg.last_success;
      const statusBit = lastError
        ? ` · <span class="err">✗ ${esc(String(lastError)).slice(0, 90)}</span>`
        : lastSuccess === true
          ? ` · <span class="ok">✓ ok</span>`
          : "";

      const card = document.createElement("div");
      card.className = "pipe-card";
      card.innerHTML =
        `<div class="pipe-head">` +
        `<span class="pipe-icon">${esc(icon)}</span>` +
        `<div class="pipe-name"><b>${esc(title)}</b>${desc ? `<span class="meta">${esc(desc)}</span>` : ""}` +
        `<span class="meta">last run: ${esc(String(lastRun))}${running}${statusBit}</span></div>` +
        `<span class="pipe-state ${enabled ? "on" : "off"}">${enabled ? "On" : "Off"}</span>` +
        `</div>`;

      const cardStatus = document.createElement("div");
      cardStatus.className = "pipe-status meta";

      const controls = document.createElement("div");
      controls.className = "row";

      const runBtn = document.createElement("button");
      runBtn.textContent = "Run now";
      runBtn.className = "primary";
      runBtn.onclick = async () => {
        const orig = runBtn.textContent;
        runBtn.disabled = true;
        runBtn.textContent = "Running…";
        cardStatus.innerHTML = `<span class="run-spin"></span> running — summarising via the model, can take 1–2 min…`;
        try {
          await api.pipeRun(name);
          cardStatus.innerHTML = `<span class="ok">✓ finished</span> — check Logs / output`;
          void logAction({
            kind: "action",
            status: "ok",
            title: `Ran pipe ${name}`,
            detail: "from Pipes tab",
            actions: [{ label: "Open folder", cmd: "open-pipe-dir", arg: name }],
          });
        } catch (e) {
          cardStatus.innerHTML = `<span class="err">✗ failed:</span> ${esc(String(e))}`;
          void logAction({ kind: "action", status: "error", title: `Run ${name}`, detail: String(e) });
        }
        runBtn.disabled = false;
        runBtn.textContent = orig;
        // refresh the list metadata after a short beat (keeps the result visible briefly)
        setTimeout(() => void refreshPipes(), 2500);
      };
      controls.appendChild(runBtn);

      const toggleBtn = document.createElement("button");
      toggleBtn.textContent = enabled ? "Disable" : "Enable";
      toggleBtn.onclick = () =>
        wrap(`${enabled ? "Disable" : "Enable"} ${name}`, () => (enabled ? api.pipeDisable(name) : api.pipeEnable(name))).then(refreshPipes);
      controls.appendChild(toggleBtn);

      const editBtn = document.createElement("button");
      editBtn.textContent = "Edit config";
      editBtn.className = "dev-only";
      editBtn.onclick = () => void openEditor(name);
      controls.appendChild(editBtn);

      const logsBtn = document.createElement("button");
      logsBtn.textContent = "Logs";
      logsBtn.className = "dev-only";
      logsBtn.onclick = async () => {
        $("p-logs").textContent = "Loading…";
        try {
          $("p-logs").textContent = (await api.pipeLogs(name)) || "(no logs)";
        } catch (e) {
          $("p-logs").textContent = `Failed: ${e}`;
        }
      };
      controls.appendChild(logsBtn);

      const openBtn = document.createElement("button");
      openBtn.textContent = "Open folder";
      openBtn.onclick = () => void api.openPipeDir(name);
      controls.appendChild(openBtn);

      const delBtn = document.createElement("button");
      delBtn.textContent = "Delete";
      delBtn.onclick = () => wrap(`Delete ${name}`, () => api.pipeDelete(name)).then(refreshPipes);
      controls.appendChild(delBtn);

      card.appendChild(controls);
      card.appendChild(scheduleControl(name, schedule));

      // Per-pipe AI preset assignment (space-separated ids = fallback chain). Advanced.
      const presetRow = document.createElement("div");
      presetRow.className = "row dev-only";
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
      card.appendChild(cardStatus);

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
