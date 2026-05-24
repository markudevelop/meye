import { $ } from "./ui";
import { api } from "./api";
import { renderMarkdown } from "./md";

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function append(role: string, text: string): HTMLElement {
  const log = $("c-log");
  const div = document.createElement("div");
  div.className = "hit";
  div.innerHTML = `<span class="meta">${esc(role)}</span><div class="c-body">${esc(text)}</div>`;
  log.appendChild(div);
  log.scrollTop = log.scrollHeight;
  return div;
}

async function send() {
  const input = $("c-input") as HTMLTextAreaElement;
  const q = input.value.trim();
  if (!q) return;
  input.value = "";
  append("you", q);
  const pending = append("meye", "…searching your recordings + asking the model…");
  const body = pending.querySelector(".c-body") as HTMLElement;
  try {
    const reply = await api.chat(q);
    body.innerHTML = renderMarkdown(reply.answer);
    if (reply.sources?.length) {
      const det = document.createElement("details");
      det.className = "sources";
      det.innerHTML = `<summary>${reply.sources.length} source${reply.sources.length > 1 ? "s" : ""} from your recordings</summary>`;
      for (const s of reply.sources) {
        const src = document.createElement("div");
        src.className = "src";
        const thumb = s.frame_id != null ? `<img src="http://127.0.0.1:3030/frames/${s.frame_id}" loading="lazy" />` : "";
        src.innerHTML = `<div class="meta">${esc(s.ts)} · ${esc(s.app)}</div><div>${esc(s.text)}</div>${thumb}`;
        det.appendChild(src);
      }
      pending.appendChild(det);
    }
  } catch (e) {
    body.textContent = `Error: ${e}`;
  }
  $("c-log").scrollTop = $("c-log").scrollHeight;
}

export function initChat() {
  $("c-send").onclick = () => void send();
  ($("c-input") as HTMLTextAreaElement).addEventListener("keydown", (e) => {
    const ke = e as KeyboardEvent;
    if (ke.key === "Enter" && (ke.metaKey || ke.ctrlKey)) {
      e.preventDefault();
      void send();
    }
  });
}
