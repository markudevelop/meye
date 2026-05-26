import { $ } from "./ui";
import { api } from "./api";

// Memory = the one place to look at everything Meye has captured. A search bar on top
// (empty = live-updating timeline feed; typing = filtered results), a small live peek of the
// current frame, and an infinite-scroll feed below. Replaces the old Live/Search/Timeline tabs.

const PAGE = 24;
let query = "";
let ctype = "all";
let offset = 0;
let loading = false;
let done = false;
let observer: IntersectionObserver | null = null;
let liveTimer: ReturnType<typeof setInterval> | null = null;
let lastLiveId: number | null = null;

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
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
  const isAudio = c.transcription != null || hit.type === "Audio" || hit.type === "audio";
  const text = String(c.text ?? c.ocr_text ?? c.transcription ?? "").slice(0, 280);
  const img = id != null && !isAudio ? `<img loading="lazy" src="http://127.0.0.1:3030/frames/${id}" />` : "";
  const kind = isAudio ? "🎙 " : "";
  return `<div class="tl-card">${img}<div class="tl-meta">${kind}${esc(fmtTime(String(ts)))} · ${esc(String(app))} ${esc(String(win))}</div>${
    text ? `<div class="tl-text">${esc(text)}</div>` : ""
  }</div>`;
}

async function loadMore() {
  if (loading || done) return;
  loading = true;
  const spinner = '<div class="loading-row"><span class="run-spin"></span> Loading…</div>';
  if (offset === 0) $("mem-feed").innerHTML = spinner;
  else $("mem-feed").insertAdjacentHTML("beforeend", `<div class="loading-row" id="mem-more"><span class="run-spin"></span></div>`);
  try {
    const res: any = await api.search({ q: query || undefined, content_type: ctype, limit: PAGE, offset });
    const data: any[] = res.data ?? res.results ?? [];
    if (offset === 0) $("mem-feed").innerHTML = "";
    document.getElementById("mem-more")?.remove();
    if (!data.length) {
      done = true;
      $("mem-end").classList.remove("hidden");
      if (offset === 0)
        $("mem-feed").innerHTML = `<p class="meta">${
          query ? "No matches for that search." : "No recordings yet — check the recorder in Settings → Recorder."
        }</p>`;
      return;
    }
    $("mem-feed").insertAdjacentHTML("beforeend", data.map(card).join(""));
    offset += data.length;
    if (data.length < PAGE) {
      done = true;
      $("mem-end").classList.remove("hidden");
    }
  } catch (e) {
    document.getElementById("mem-more")?.remove();
    if (offset === 0) $("mem-feed").innerHTML = "";
    $("mem-feed").insertAdjacentHTML("beforeend", `<p class="warn">Couldn't load: ${esc(String(e))} (is the recorder running?)</p>`);
    done = true;
  } finally {
    loading = false;
  }
}

function resetFeed() {
  offset = 0;
  done = false;
  $("mem-feed").innerHTML = "";
  $("mem-end").classList.add("hidden");
  void loadMore();
}

/** Run a search in Memory (used by voice commands and the command palette). */
export function searchMemory(q: string) {
  query = q.trim();
  ($("mem-q") as HTMLInputElement).value = query;
  resetFeed();
}

async function tickLive() {
  try {
    const res: any = await api.search({ content_type: "ocr", limit: 1 });
    const f = (res.data ?? [])[0];
    if (!f) return;
    const c = f.content ?? f;
    const id = c.frame_id ?? c.frameId ?? c.id ?? null;
    if (id === lastLiveId) return;
    lastLiveId = id;
    const app = c.app_name ?? "";
    const ts = c.timestamp ?? "";
    $("mem-live").innerHTML =
      id != null
        ? `<div class="mem-live-card" title="What Meye is capturing right now"><img src="http://127.0.0.1:3030/frames/${id}" /><div class="mem-live-meta"><span class="live-pulse"></span> Live · ${esc(fmtTime(String(ts)))} · ${esc(String(app))}</div></div>`
        : "";
  } catch {
    /* recorder down — leave the peek as-is */
  }
}

export function startMemory() {
  if (offset === 0 && !$("mem-feed").children.length) resetFeed();
  if (!liveTimer) {
    lastLiveId = null;
    void tickLive();
    liveTimer = setInterval(() => void tickLive(), 4000);
  }
}

export function stopMemory() {
  if (liveTimer) {
    clearInterval(liveTimer);
    liveTimer = null;
  }
}

export function initMemory() {
  const q = $("mem-q") as HTMLInputElement;
  let deb: ReturnType<typeof setTimeout> | undefined;
  q.addEventListener("input", () => {
    clearTimeout(deb);
    deb = setTimeout(() => {
      query = q.value.trim();
      resetFeed();
    }, 350);
  });
  q.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") {
      query = q.value.trim();
      resetFeed();
    }
  });
  ($("mem-type") as HTMLSelectElement).onchange = (e) => {
    ctype = (e.target as HTMLSelectElement).value;
    resetFeed();
  };
  observer = new IntersectionObserver((en) => en[0].isIntersecting && void loadMore(), { root: null, rootMargin: "500px" });
  observer.observe($("mem-sentinel"));
}
