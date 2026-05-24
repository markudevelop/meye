import { $ } from "./ui";
import { api } from "./api";

const PAGE = 24;
let offset = 0;
let loading = false;
let done = false;
let observer: IntersectionObserver | null = null;

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function fmtTime(ts: string): string {
  const d = new Date(ts);
  return isNaN(d.getTime()) ? ts : d.toLocaleString();
}

function card(hit: any): string {
  const c = hit.content ?? hit;
  const id = c.frame_id ?? c.frameId ?? c.id;
  const ts = c.timestamp ?? c.created_at ?? "";
  const app = c.app_name ?? c.appName ?? "";
  const win = c.window_name ?? c.windowName ?? "";
  const text = String(c.text ?? c.ocr_text ?? c.transcription ?? "").slice(0, 280);
  const img = id != null ? `<img loading="lazy" src="http://127.0.0.1:3030/frames/${id}" />` : "";
  return `<div class="tl-card">${img}<div class="tl-meta">${esc(fmtTime(String(ts)))} · ${esc(String(app))} ${esc(String(win))}</div>${
    text ? `<div class="tl-text">${esc(text)}</div>` : ""
  }</div>`;
}

async function loadMore() {
  if (loading || done) return;
  loading = true;
  try {
    const res = await api.search({ content_type: "ocr", limit: PAGE, offset });
    const data: any[] = res.data ?? res.results ?? [];
    if (!data.length) {
      done = true;
      $("t-end").classList.remove("hidden");
      if (offset === 0) $("t-feed").innerHTML = "<p class='meta'>No frames yet (is the recorder running? see Status).</p>";
      return;
    }
    $("t-feed").insertAdjacentHTML("beforeend", data.map(card).join(""));
    offset += data.length;
    if (data.length < PAGE) {
      done = true;
      $("t-end").classList.remove("hidden");
    }
  } catch (e) {
    $("t-feed").insertAdjacentHTML("beforeend", `<p class="warn">Failed to load: ${esc(String(e))}</p>`);
    done = true;
  } finally {
    loading = false;
  }
}

function reset() {
  offset = 0;
  done = false;
  $("t-feed").innerHTML = "";
  $("t-end").classList.add("hidden");
  void loadMore();
}

export function initTimeline() {
  $("t-refresh").onclick = () => reset();
  // Fire loadMore as the sentinel nears the viewport (works while .main scrolls).
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0].isIntersecting) void loadMore();
    },
    { root: null, rootMargin: "500px" }
  );
  observer.observe($("t-sentinel"));
}

export function loadTimelineIfEmpty() {
  if (offset === 0 && !$("t-feed").children.length) reset();
}
