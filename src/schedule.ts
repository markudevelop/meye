import { $, wrap, toast } from "./ui";
import { api } from "./api";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
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
export function scheduleKey(schedule: string): string {
  const s = (schedule ?? "").trim();
  const hit = PRESETS.find((p) => p.key !== "custom" && p.schedule === s);
  return hit ? hit.key : "custom";
}

function optionsHtml(selectedKey: string): string {
  return PRESETS.map(
    (p) => `<option value="${p.key}"${p.key === selectedKey ? " selected" : ""}>${p.label}</option>`
  ).join("");
}

export async function refreshSchedule() {
  const out = $("sched-list");
  try {
    const pipes = (await api.pipeList()) as any[];
    if (!pipes.length) {
      out.innerHTML = "<p class='meta'>No pipes installed. Add some from the Pipes tab.</p>";
      return;
    }
    out.innerHTML = "";
    for (const p of pipes) {
      const c = p.config ?? {};
      const name = String(c.name ?? "");
      if (!name) continue;
      const title = String(c.title ?? name);
      const icon = String(c.icon ?? "⚙️");
      const enabled = c.enabled === true;
      const schedule = String(c.schedule ?? "manual");
      const key = scheduleKey(schedule);
      const lastErr = p.last_error
        ? ` · <span class="err">✗ ${esc(String(p.last_error)).slice(0, 80)}</span>`
        : p.last_success
          ? ` · <span class="ok">✓ ran</span>`
          : "";

      const card = document.createElement("div");
      card.className = "card sched-row";
      card.innerHTML =
        `<div class="sched-main">` +
        `<label class="switch"><input type="checkbox" class="sched-enabled"${enabled ? " checked" : ""} /><span class="slider"></span></label>` +
        `<div class="sched-name"><b>${icon} ${esc(title)}</b><span class="meta">${esc(name)}${lastErr}</span></div>` +
        `<select class="sched-select">${optionsHtml(key)}</select>` +
        `</div>` +
        `<input class="sched-custom${key === "custom" ? "" : " hidden"}" placeholder="cron or 'every 30m'" value="${key === "custom" ? esc(schedule) : ""}" />`;

      const toggle = card.querySelector(".sched-enabled") as HTMLInputElement;
      const sel = card.querySelector(".sched-select") as HTMLSelectElement;
      const custom = card.querySelector(".sched-custom") as HTMLInputElement;

      toggle.onchange = () =>
        wrap(toggle.checked ? `Enable ${name}` : `Disable ${name}`, () =>
          toggle.checked ? api.pipeEnable(name) : api.pipeDisable(name)
        ).then(() => void refreshSchedule());

      sel.onchange = () => {
        const k = sel.value;
        if (k === "custom") {
          custom.classList.remove("hidden");
          custom.focus();
          return; // wait for the user to type + commit
        }
        custom.classList.add("hidden");
        const schedStr = PRESETS.find((x) => x.key === k)!.schedule;
        void applySchedule(name, schedStr);
      };

      // Custom cron applies on Enter or blur — never per keystroke.
      const commitCustom = () => {
        const v = custom.value.trim();
        if (v) void applySchedule(name, v);
      };
      custom.onkeydown = (e) => {
        if ((e as KeyboardEvent).key === "Enter") {
          e.preventDefault();
          commitCustom();
        }
      };
      custom.onblur = commitCustom;

      out.appendChild(card);
    }
  } catch (e) {
    out.innerHTML = `<p class="warn">Failed to load pipes: ${esc(String(e))} (is the recorder running? see Status tab)</p>`;
  }
}

async function applySchedule(name: string, schedule: string) {
  await wrap(`Schedule ${name} → ${schedule}`, () => api.pipeSetSchedule(name, schedule));
  toast(`${name}: ${schedule}`);
  void refreshSchedule();
}

export function initSchedule() {
  $("sched-refresh").onclick = () => void refreshSchedule();
}
