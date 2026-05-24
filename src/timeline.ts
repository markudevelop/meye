import { $, wrap } from "./ui";
import { api } from "./api";

type Frame = { id: number; ts: string; app: string; win: string };
let frames: Frame[] = [];

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

async function show(i: number) {
  const f = frames[i];
  if (!f) return;
  ($("t-img") as HTMLImageElement).src = `http://127.0.0.1:3030/frames/${f.id}`;
  $("t-meta").textContent = `${f.ts} · ${f.app} ${f.win} (frame ${f.id} · ${i + 1}/${frames.length})`;
  $("t-ocr").textContent = "…";
  try {
    const ocr = await api.frameOcr(f.id);
    const text = (ocr && (ocr.text ?? ocr.ocr_text)) || JSON.stringify(ocr);
    $("t-ocr").textContent = String(text) || "(no ocr)";
  } catch {
    $("t-ocr").textContent = "(no ocr)";
  }
}

async function load() {
  await wrap("Load timeline", async () => {
    const limit = Number(($("t-limit") as HTMLInputElement).value) || 200;
    const res = await api.search({ content_type: "ocr", limit });
    const data: any[] = res.data ?? res.results ?? [];
    frames = data.map((h) => pick(h.content ?? h)).filter((x): x is Frame => !!x);
    frames.reverse(); // oldest → newest, so the slider reads left=old, right=recent
    const slider = $("t-slider") as HTMLInputElement;
    slider.max = String(Math.max(0, frames.length - 1));
    slider.value = String(Math.max(0, frames.length - 1));
    if (frames.length) {
      await show(frames.length - 1);
    } else {
      $("t-meta").textContent = "No frames found (is the recorder running? see Status).";
      ($("t-img") as HTMLImageElement).removeAttribute("src");
      $("t-ocr").textContent = "—";
    }
  });
}

export function initTimeline() {
  $("t-load").onclick = () => void load();
  ($("t-slider") as HTMLInputElement).oninput = (e) => void show(Number((e.target as HTMLInputElement).value));
}

export function loadTimelineIfEmpty() {
  if (!frames.length) void load();
}
