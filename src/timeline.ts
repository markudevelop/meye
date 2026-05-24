import { $, wrap } from "./ui";
import { api } from "./api";

type Frame = { id: number; ts: string; app: string; win: string };
let frames: Frame[] = [];
let idx = 0;
let timer: number | null = null;

function pick(c: any): Frame | null {
  const id = c.frame_id ?? c.frameId ?? c.id;
  if (id == null) return null;
  return {
    id: Number(id),
    ts: String(c.timestamp ?? c.created_at ?? ""),
    app: String(c.app_name ?? c.appName ?? ""),
    win: String(c.window_name ?? c.windowName ?? ""),
  };
}

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function fmtTime(ts: string): string {
  const d = new Date(ts);
  return isNaN(d.getTime()) ? ts : d.toLocaleString();
}

async function show(i: number) {
  if (!frames.length) return;
  idx = Math.max(0, Math.min(frames.length - 1, i));
  const f = frames[idx];
  ($("t-img") as HTMLImageElement).src = `http://127.0.0.1:3030/frames/${f.id}`;
  $("t-meta").innerHTML = `<b>${esc(fmtTime(f.ts))}</b> · ${esc(f.app)} ${esc(f.win)} · ${idx + 1}/${frames.length}`;
  ($("t-slider") as HTMLInputElement).value = String(idx);
  const strip = $("t-strip");
  Array.from(strip.children).forEach((el, j) => (el as HTMLElement).classList.toggle("active", j === idx));
  (strip.children[idx] as HTMLElement | undefined)?.scrollIntoView({ inline: "center", block: "nearest" });
  $("t-ocr").textContent = "…";
  try {
    const ocr = await api.frameOcr(f.id);
    const text = (ocr && (ocr.text ?? ocr.ocr_text)) || JSON.stringify(ocr);
    $("t-ocr").textContent = String(text) || "(no ocr)";
  } catch {
    $("t-ocr").textContent = "(no ocr)";
  }
}

function renderStrip() {
  const strip = $("t-strip");
  strip.innerHTML = "";
  frames.forEach((f, j) => {
    const im = document.createElement("img");
    im.className = "tn";
    im.loading = "lazy";
    im.src = `http://127.0.0.1:3030/frames/${f.id}`;
    im.onclick = () => {
      stop();
      void show(j);
    };
    strip.appendChild(im);
  });
}

async function load() {
  await wrap("Load timeline", async () => {
    const limit = Number(($("t-limit") as HTMLInputElement).value) || 150;
    const res = await api.search({ content_type: "ocr", limit });
    const data: any[] = res.data ?? res.results ?? [];
    frames = data.map((h) => pick(h.content ?? h)).filter((x): x is Frame => !!x);
    frames.reverse(); // oldest → newest
    ($("t-slider") as HTMLInputElement).max = String(Math.max(0, frames.length - 1));
    if (frames.length) {
      renderStrip();
      await show(frames.length - 1);
    } else {
      $("t-meta").textContent = "No frames found (is the recorder running? see Status).";
      ($("t-img") as HTMLImageElement).removeAttribute("src");
      $("t-strip").innerHTML = "";
      $("t-ocr").textContent = "—";
    }
  });
}

function play() {
  if (timer != null || !frames.length) return;
  $("t-play").textContent = "⏸ Pause";
  timer = window.setInterval(() => {
    if (idx >= frames.length - 1) {
      stop();
      return;
    }
    void show(idx + 1);
  }, 700);
}

function stop() {
  if (timer != null) {
    clearInterval(timer);
    timer = null;
  }
  $("t-play").textContent = "▶ Play";
}

export function initTimeline() {
  $("t-load").onclick = () => void load();
  $("t-prev").onclick = () => {
    stop();
    void show(idx - 1);
  };
  $("t-next").onclick = () => {
    stop();
    void show(idx + 1);
  };
  $("t-play").onclick = () => (timer == null ? play() : stop());
  ($("t-slider") as HTMLInputElement).oninput = (e) => {
    stop();
    void show(Number((e.target as HTMLInputElement).value));
  };
  document.addEventListener("keydown", (e) => {
    if ($("panel-timeline").classList.contains("hidden")) return;
    const ke = e as KeyboardEvent;
    if (ke.key === "ArrowLeft") {
      stop();
      void show(idx - 1);
    } else if (ke.key === "ArrowRight") {
      stop();
      void show(idx + 1);
    }
  });
}

export function loadTimelineIfEmpty() {
  if (!frames.length) void load();
}
