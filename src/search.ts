import { $ } from "./ui";
import { api } from "./api";

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function row(hit: any): string {
  // screenpipe /search returns { type, content: {...} }; content shape varies by type.
  const c = hit.content ?? hit;
  const text = c.text ?? c.transcription ?? c.ocr_text ?? "(no text)";
  const app = c.app_name ?? c.appName ?? "";
  const win = c.window_name ?? c.windowName ?? "";
  const ts = c.timestamp ?? c.created_at ?? "";
  const frameId = c.frame_id ?? c.frameId ?? c.id;
  const isOcr = hit.type === "OCR" || hit.type === "ocr" || c.ocr_text != null;
  const img = isOcr && frameId != null ? `<img src="http://127.0.0.1:3030/frames/${frameId}" loading="lazy" />` : "";
  return `<div class="hit"><div>${escapeHtml(String(text)).slice(0, 600)}</div>
    <div class="meta">${escapeHtml(String(app))} · ${escapeHtml(String(win))} · ${escapeHtml(String(ts))} · ${escapeHtml(String(hit.type ?? ""))}</div>${img}</div>`;
}

async function runSearch() {
  const params = {
    q: ($("s-q") as HTMLInputElement).value,
    content_type: ($("s-type") as HTMLSelectElement).value,
    app_name: ($("s-app") as HTMLInputElement).value,
    limit: Number(($("s-limit") as HTMLInputElement).value) || 50,
  };
  const out = $("s-results");
  out.textContent = "Searching…";
  try {
    const res = await api.search(params);
    const data: any[] = res.data ?? res.results ?? [];
    out.innerHTML = data.length
      ? data.map(row).join("")
      : "<p class='meta'>No results (or recorder not running — see Status).</p>";
  } catch (e) {
    out.innerHTML = `<p class='warn'>Search failed: ${escapeHtml(String(e))}. Is the recorder running? Check the Status tab.</p>`;
  }
}

/** Run a search with a given query (used by voice commands). */
export function runSearchWith(query: string) {
  ($("s-q") as HTMLInputElement).value = query;
  ($("s-type") as HTMLSelectElement).value = "all";
  void runSearch();
}

export function initSearch() {
  $("s-go").onclick = () => void runSearch();
  ($("s-q") as HTMLInputElement).addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") void runSearch();
  });
}
