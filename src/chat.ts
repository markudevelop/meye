import { $ } from "./ui";
import { api } from "./api";

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
    body.textContent = await api.chat(q);
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
