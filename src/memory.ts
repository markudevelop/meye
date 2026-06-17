import { $ } from "./ui";
import { api } from "./api";
import { renderMarkdown } from "./md";

// Memory = the one place to look at everything Meye has captured. A search bar on top
// (empty = live-updating timeline feed; typing = filtered results), a small live peek of the
// current frame, and an infinite-scroll feed below. Replaces the old Live/Search/Timeline tabs.

const PAGE = 24;
// screenpipe's /search gathers every row in the [start_time, end_time] window (up to its
// internal 10k cap) BEFORE applying our limit — and each frames row drags a large
// accessibility-tree blob off disk. With no window it scans the whole DB → multi-minute
// queries that exhaust the connection pool. So the browse feed pages BACKWARD in bounded
// time windows: each request only ever scans ~WINDOW_MS of history, never the whole table.
const WINDOW_MS = 6 * 60 * 60 * 1000; // 6h gather window per page
const FLOOR_MS = 31 * 24 * 60 * 60 * 1000; // stop paging past ~retention; nothing older is kept
const isoFrom = (ms: number) => new Date(ms).toISOString();

let query = "";
let ctype = "all";
let offset = 0; // used only by the keyword-search path (FTS-indexed, fast)
let cursorEnd: number | null = null; // browse path: end_time (ms epoch) for the next page; null = now
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

/** Keyword search path — FTS-indexed, so plain offset paging stays fast. */
async function loadSearch(): Promise<any[]> {
  const res: any = await api.search({ q: query, content_type: ctype, limit: PAGE, offset });
  const data: any[] = res.data ?? res.results ?? [];
  offset += data.length;
  if (data.length < PAGE) {
    done = true;
    $("mem-end").classList.remove("hidden");
  }
  return data;
}

/** Browse path — no query. Page backward through history in bounded time windows so
 * screenpipe only ever scans ~WINDOW_MS of rows per request, never the whole table. */
async function loadBrowse(): Promise<any[]> {
  let end = cursorEnd ?? Date.now();
  const floor = Date.now() - FLOOR_MS;
  // Step back over inactive gaps (screen locked, off-hours) until a window has rows.
  for (let step = 0; step < 30 && end > floor; step++) {
    const start = Math.max(end - WINDOW_MS, floor);
    const res: any = await api.search({ content_type: ctype, start_time: isoFrom(start), end_time: isoFrom(end), limit: PAGE });
    const data: any[] = res.data ?? res.results ?? [];
    if (data.length) {
      const oldest = data[data.length - 1];
      const ts = new Date((oldest.content ?? oldest).timestamp ?? "").getTime();
      cursorEnd = isNaN(ts) ? start : ts - 1; // exclusive: next page resumes just before the oldest shown
      if (cursorEnd <= floor) {
        done = true;
        $("mem-end").classList.remove("hidden");
      }
      return data;
    }
    end = start; // empty window — keep stepping back
  }
  done = true;
  $("mem-end").classList.remove("hidden");
  return [];
}

async function loadMore() {
  if (loading || done) return;
  loading = true;
  const fresh = !$("mem-feed").querySelector(".tl-card"); // no cards yet → first page of this query
  const spinner = `<div class="loading-row"><span class="run-spin"></span> ${query ? "Searching your memory…" : "Loading your memory…"}</div>`;
  if (fresh) $("mem-feed").innerHTML = spinner;
  else $("mem-feed").insertAdjacentHTML("beforeend", `<div class="loading-row" id="mem-more"><span class="run-spin"></span></div>`);
  try {
    const data: any[] = query ? await loadSearch() : await loadBrowse();
    if (fresh) $("mem-feed").innerHTML = "";
    document.getElementById("mem-more")?.remove();
    if (!data.length) {
      if (fresh)
        $("mem-feed").innerHTML = query
          ? `<div class="empty-state"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3-3"/></svg><div class="es-title">No matches for "${esc(query)}"</div><div class="es-sub">Try fewer or different words.</div></div>`
          : `<div class="empty-state"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg><div class="es-title">Nothing here yet</div><div class="es-sub">Your memory fills up as you work. Make sure recording is on in Settings → Recorder.</div></div>`;
      return;
    }
    $("mem-feed").insertAdjacentHTML("beforeend", data.map(card).join(""));
  } catch (e) {
    document.getElementById("mem-more")?.remove();
    if (fresh) $("mem-feed").innerHTML = "";
    $("mem-feed").insertAdjacentHTML("beforeend", `<p class="warn">Couldn't load: ${esc(String(e))} (is the recorder running?)</p>`);
    done = true;
  } finally {
    loading = false;
  }
}

function resetFeed() {
  offset = 0;
  cursorEnd = null;
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
    // Bounded to the last 2h so this 4s tick can never trigger a whole-DB scan.
    const res: any = await api.search({ content_type: "ocr", limit: 1, start_time: isoFrom(Date.now() - 2 * 60 * 60 * 1000) });
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

// Auto-surface new recordings: only when not searching, not loading, and the user is parked at
// the top — so we never yank the page while they scroll or search.
let feedTimer: ReturnType<typeof setInterval> | null = null;
function maybeRefreshFeed() {
  const main = document.querySelector(".main") as HTMLElement | null;
  if (query === "" && !loading && main && main.scrollTop < 40) resetFeed();
}

export function startMemory() {
  if (offset === 0 && !$("mem-feed").children.length) resetFeed();
  if (!liveTimer) {
    lastLiveId = null;
    void tickLive();
    liveTimer = setInterval(() => void tickLive(), 4000);
  }
  if (!feedTimer) feedTimer = setInterval(maybeRefreshFeed, 12000);
}

export function stopMemory() {
  if (liveTimer) {
    clearInterval(liveTimer);
    liveTimer = null;
  }
  if (feedTimer) {
    clearInterval(feedTimer);
    feedTimer = null;
  }
}

/** Generate an AI recap of today's activity from the recordings (on demand). */
async function recapDay() {
  const out = $("mem-today-out");
  const btn = $("mem-today-btn") as HTMLButtonElement;
  btn.disabled = true;
  out.innerHTML = '<div class="loading-row"><span class="run-spin"></span> Summarizing your day…</div>';
  try {
    const r = await api.chat(
      "Give me a concise recap of what I worked on and did today, based on my screen and audio recordings. A few short bullet points grouped by topic; skip anything you have no data for."
    );
    out.innerHTML = `<div class="md mem-today-md">${renderMarkdown(r.answer)}</div>`;
  } catch (e) {
    out.innerHTML = `<p class="warn">Couldn't generate a recap: ${esc(String(e))} (set an AI model in Settings → AI).</p>`;
  }
  btn.disabled = false;
}

export function initMemory() {
  ($("mem-today-btn") as HTMLButtonElement).onclick = () => void recapDay();
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
