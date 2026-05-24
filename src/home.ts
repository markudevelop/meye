import { $ } from "./ui";
import { api } from "./api";
import { goTab } from "./tabs";

const PRESETS: Record<string, string[]> = {
  saver: [
    "--audio-transcription-engine",
    "whisper-large-v3-turbo-quantized",
    "--disable-meeting-detector",
    "--disable-clipboard-capture",
    "--idle-capture-interval-ms",
    "120000",
  ],
  balanced: ["--disable-clipboard-capture", "--idle-capture-interval-ms", "60000"],
  performance: [],
};

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function fmtTime(ts: number): string {
  const d = new Date(ts);
  return isNaN(d.getTime()) ? "" : d.toLocaleTimeString();
}

type FollowUp = { label: string; cmd: string; arg?: string };

function followupRow(actions: FollowUp[]): HTMLElement {
  const row = document.createElement("div");
  row.className = "row";
  for (const a of actions) {
    const b = document.createElement("button");
    b.textContent = a.label;
    b.onclick = () => {
      if (a.cmd === "open-pipe-dir") void api.openPipeDir(a.arg!);
      else if (a.cmd === "go-tab") goTab(a.arg as any);
    };
    row.appendChild(b);
  }
  return row;
}

function renderEntry(e: any): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "msg " + (e.kind ?? "");
  const time = `<span class="msg-time">${fmtTime(e.ts ?? Date.now())}</span>`;
  if (e.kind === "user") {
    wrap.innerHTML = `<div class="bubble user">${esc(e.text ?? "")}</div>`;
    return wrap;
  }
  if (e.kind === "assistant") {
    wrap.innerHTML = `<div class="bubble meye"><div class="who">Meye ${time}</div><div class="body">${esc(e.text ?? "")}</div></div>`;
    if (e.sources?.length) {
      const det = document.createElement("details");
      det.className = "sources";
      det.innerHTML =
        `<summary>${e.sources.length} sources from your recordings</summary>` +
        e.sources
          .map(
            (s: any) =>
              `<div class="src"><div class="meta">${esc(String(s.ts ?? ""))} · ${esc(String(s.app ?? ""))}</div><div>${esc(
                String(s.text ?? "").slice(0, 200)
              )}</div>${s.frame_id != null ? `<img loading="lazy" src="http://127.0.0.1:3030/frames/${s.frame_id}" />` : ""}</div>`
          )
          .join("");
      wrap.querySelector(".bubble")!.appendChild(det);
    }
    return wrap;
  }
  // action card
  const icon =
    e.status === "ok"
      ? `<span class="ok">✓</span>`
      : e.status === "error"
        ? `<span class="err">✗</span>`
        : `<span class="run-spin"></span>`;
  wrap.innerHTML = `<div class="bubble action-card">${icon} <b>${esc(e.title ?? "")}</b> ${time}${
    e.detail ? `<div class="meta">${esc(e.detail)}</div>` : ""
  }</div>`;
  if (e.status === "ok" && e.actions?.length) {
    wrap.querySelector(".bubble")!.appendChild(followupRow(e.actions));
  }
  return wrap;
}

function feed(): HTMLElement {
  return $("home-feed");
}

function add(e: any): HTMLElement {
  const node = renderEntry(e);
  feed().appendChild(node);
  feed().scrollTop = feed().scrollHeight;
  return node;
}

async function persist(e: any) {
  try {
    await api.activityAppend(e);
  } catch {
    /* non-fatal */
  }
}

async function submit() {
  const input = $("home-input") as HTMLTextAreaElement;
  const text = input.value.trim();
  if (!text) return;
  input.value = "";
  const userEntry = { kind: "user", ts: Date.now(), text };
  add(userEntry);
  await persist(userEntry);

  if (text.startsWith("/run ")) return doAction("run", text.slice(5).trim());
  if (text.startsWith("/search ")) return doAction("search", text.slice(8).trim());
  if (text.startsWith("/profile ")) return doAction("profile", text.slice(9).trim());
  return doChat(text);
}

async function doChat(question: string) {
  const placeholder = add({ kind: "assistant", ts: Date.now(), text: "…thinking…" });
  try {
    const reply = await api.chat(question);
    const entry = { kind: "assistant", ts: Date.now(), text: reply.answer, sources: reply.sources };
    placeholder.replaceWith(renderEntry(entry));
    await persist(entry);
  } catch (e) {
    const entry = { kind: "assistant", ts: Date.now(), text: `Error: ${e}` };
    placeholder.replaceWith(renderEntry(entry));
    await persist(entry);
  }
  feed().scrollTop = feed().scrollHeight;
}

async function doAction(kind: "run" | "search" | "profile", arg: string) {
  const titles = { run: `Run pipe ${arg}`, search: `Search "${arg}"`, profile: `Apply ${arg} profile` };
  const placeholder = add({ kind: "action", ts: Date.now(), status: "running", title: titles[kind] });
  let entry: any;
  try {
    if (kind === "run") {
      await api.pipeRun(arg);
      entry = {
        kind: "action",
        ts: Date.now(),
        status: "ok",
        title: `Ran pipe ${arg}`,
        detail: "finished",
        actions: [
          { label: "Open folder", cmd: "open-pipe-dir", arg },
          { label: "Pipes tab", cmd: "go-tab", arg: "pipes" },
        ],
      };
    } else if (kind === "search") {
      const res = await api.search({ q: arg, content_type: "all", limit: 20 });
      const n = (res.data ?? res.results ?? []).length;
      entry = {
        kind: "action",
        ts: Date.now(),
        status: "ok",
        title: `Searched "${arg}"`,
        detail: `${n} results`,
        actions: [{ label: "Open Search", cmd: "go-tab", arg: "search" }],
      };
    } else {
      const preset = PRESETS[arg] ?? null;
      if (!preset) throw new Error("unknown profile (saver|balanced|performance)");
      await api.setRecordArgs(preset);
      entry = { kind: "action", ts: Date.now(), status: "ok", title: `Applied ${arg} profile`, detail: "recorder restarted" };
    }
  } catch (e) {
    entry = { kind: "action", ts: Date.now(), status: "error", title: titles[kind], detail: String(e) };
  }
  placeholder.replaceWith(renderEntry(entry));
  await persist(entry);
  feed().scrollTop = feed().scrollHeight;
}

let loaded = false;

export async function loadHome() {
  if (loaded) return;
  loaded = true;
  const entries = await api.activityRead().catch(() => [] as any[]);
  feed().innerHTML = "";
  if (!entries.length) {
    feed().innerHTML = `<p class="meta home-hello">👋 Ask about your day, or run an action. Try <code>/run obsidian-sync</code>, <code>/search error</code>, or just type a question.</p>`;
  } else {
    for (const e of entries) feed().appendChild(renderEntry(e));
  }
  feed().scrollTop = feed().scrollHeight;
}

export function initHome() {
  $("home-send").onclick = () => void submit();
  ($("home-input") as HTMLTextAreaElement).addEventListener("keydown", (e) => {
    const ke = e as KeyboardEvent;
    if (ke.key === "Enter" && !ke.shiftKey) {
      e.preventDefault();
      void submit();
    }
  });
  $("home-clear").onclick = async () => {
    await api.activityClear();
    loaded = false;
    void loadHome();
  };
}
