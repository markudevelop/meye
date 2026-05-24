import { $, wrap } from "./ui";
import { api } from "./api";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
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

      card.appendChild(controls);
      out.appendChild(card);
    }
  } catch (e) {
    out.innerHTML = `<p class="warn">Failed to load pipes: ${esc(String(e))} (is the recorder running? see Status tab)</p>`;
  }
}

export function initPipes() {
  $("p-refresh").onclick = () => void refreshPipes();
}
