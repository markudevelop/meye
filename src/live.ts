import { $ } from "./ui";
import { api } from "./api";

// Live view: shows what the recorder is capturing right now — the newest screen frame and the
// most recent audio transcripts — plus which sources are on. Polls every 3s while visible.

let timer: ReturnType<typeof setInterval> | null = null;
let lastFrameId: number | null = null;

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] as string);
}
function fmtTime(ts: string): string {
  const d = new Date(ts);
  return isNaN(d.getTime()) ? ts : d.toLocaleTimeString();
}
function pill(label: string, on: boolean): string {
  return `<span class="live-pill ${on ? "on" : "off"}">${label} ${on ? "●" : "○"}</span>`;
}

async function tick() {
  try {
    const [frameRes, audioRes, args, h] = await Promise.all([
      api.search({ content_type: "ocr", limit: 1 }) as Promise<any>,
      api.search({ content_type: "audio", limit: 6 }) as Promise<any>,
      api.getRecordArgs().catch(() => [] as string[]),
      api.getHealth().catch(() => null) as Promise<any>,
    ]);

    // Actual runtime status from /health is the source of truth — args can say "audio on"
    // while the OS blocks it (e.g. Microphone permission reset), in which case audio_status
    // is "disabled" and frame_status goes "stale".
    const audioDisabled = (h?.audio_status ?? "") === "disabled";
    const frameStale = !!h?.frame_status && h.frame_status !== "ok";

    // Which sources are configured (record args) AND actually working (health).
    const video = !args.includes("--disable-vision");
    const audioOff = args.includes("--disable-audio") || audioDisabled;
    const devs: string[] = [];
    for (let i = 0; i < args.length; i++) if (args[i] === "--audio-device") devs.push(args[i + 1] ?? "");
    const micOn = !audioOff && (devs.length === 0 || devs.some((d) => /input|microphone|mic/i.test(d)));
    const pcOn = !audioOff && (devs.length === 0 || devs.some((d) => /output|system/i.test(d)));
    $("live-sources").innerHTML = pill("🖥 Screen", video && !frameStale) + pill("🎙 Microphone", micOn) + pill("🔊 Computer audio", pcOn);

    const anyOn = (video && !frameStale) || micOn || pcOn;
    $("live-badge").innerHTML = `<span class="dot ${anyOn ? "green" : "grey"}"></span><span>${anyOn ? "Recording" : "Not capturing"}</span>`;

    // Latest screen frame (only rebuild the <img> when the frame id changes, to avoid flicker).
    const f = (frameRes.data ?? [])[0];
    if (!video) {
      lastFrameId = null;
      $("live-frame").innerHTML = "<div class='empty-state'><div class='es-title'>Screen capture is off</div><div class='es-sub'>Turn it on in Performance → Capture sources.</div></div>";
    } else if (frameStale) {
      lastFrameId = null;
      $("live-frame").innerHTML = "<div class='empty-state'><div class='es-title'>⚠ Screen capture stalled</div><div class='es-sub'>The recorder isn't getting fresh frames — Screen Recording permission was likely reset. Re-grant it (Status tab), then Stop/Start.</div></div>";
    } else if (f) {
      const c = f.content ?? f;
      const id = c.frame_id ?? c.frameId ?? c.id ?? null;
      if (id !== lastFrameId) {
        lastFrameId = id;
        const cap = `${esc(fmtTime(String(c.timestamp ?? "")))} · ${esc(String(c.app_name ?? ""))} ${esc(String(c.window_name ?? ""))}`;
        $("live-frame").innerHTML =
          id != null
            ? `<img src="http://127.0.0.1:3030/frames/${id}" /><div class="live-cap">${cap}</div>`
            : "<p class='meta'>No frames captured yet.</p>";
      }
    }

    // Recent audio transcripts.
    const lines = ((audioRes.data ?? []) as any[])
      .map((hit) => {
        const c = hit.content ?? hit;
        return { ts: String(c.timestamp ?? ""), text: String(c.transcription ?? c.text ?? "").trim() };
      })
      .filter((l) => l.text);
    if (audioDisabled) {
      $("live-audio").innerHTML =
        "<div class='empty-state'><div class='es-title'>Audio capture is off</div><div class='es-sub'>macOS reports audio disabled — the Microphone permission was likely reset. Re-grant it in System Settings → Privacy → Microphone, then Status → Stop/Start. Or enable a source in Performance → Capture sources.</div></div>";
    } else if (audioOff) {
      $("live-audio").innerHTML =
        "<div class='empty-state'><div class='es-title'>Audio capture is off</div><div class='es-sub'>Turn on Microphone or Computer audio in Performance.</div></div>";
    } else if (lines.length) {
      $("live-audio").innerHTML = lines
        .map((l) => `<div class="live-line"><span class="live-line-time">${esc(fmtTime(l.ts))}</span> ${esc(l.text)}</div>`)
        .join("");
    } else {
      $("live-audio").innerHTML = "<p class='meta'>Listening… nothing transcribed in the last clips.</p>";
    }
  } catch {
    $("live-frame").innerHTML = "<p class='warn'>Can't reach the recorder — check the Status tab.</p>";
  }
}

export function startLive() {
  if (timer) return;
  lastFrameId = null;
  void tick();
  timer = setInterval(() => void tick(), 3000);
}

export function stopLive() {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
}

export function initLive() {
  /* polling is started/stopped on tab show via main.ts */
}
