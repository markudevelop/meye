import { api } from "./api";
import { toast } from "./ui";
import { goTab, type Tab } from "./tabs";
import { startNewChat } from "./home";
import { setVisionPaused } from "./performance";
import { runSearchWith } from "./search";

// Voice commands ride the local mic transcript screenpipe already produces — no new capture,
// no cloud. We poll the latest audio transcription, send it to the Rust parser, and dispatch
// any recognised "Hey Meye …" command to the existing UI actions.

const KEY = "meye.voice";
let timer: ReturnType<typeof setInterval> | null = null;
let lastTs = "";

function dispatch(cmd: { action: string; arg: string }) {
  switch (cmd.action) {
    case "pause":
      void setVisionPaused(true);
      toast("🎙 Paused screen capture");
      break;
    case "resume":
      void setVisionPaused(false);
      toast("🎙 Resumed screen capture");
      break;
    case "new-chat":
      startNewChat();
      toast("🎙 New chat");
      break;
    case "open":
      goTab(cmd.arg as Tab);
      toast(`🎙 Opened ${cmd.arg}`);
      break;
    case "search":
      goTab("search");
      runSearchWith(cmd.arg);
      toast(`🎙 Searching "${cmd.arg}"`);
      break;
  }
}

async function tick() {
  try {
    const res: any = await api.search({ content_type: "audio", limit: 5 });
    const data: any[] = res.data ?? res.results ?? [];
    const hits = data
      .map((h) => {
        const c = h.content ?? h;
        return {
          ts: String(c.timestamp ?? c.created_at ?? ""),
          text: String(c.transcription ?? c.text ?? ""),
          dev: String(c.device_name ?? c.deviceName ?? ""),
        };
      })
      .filter((h) => h.text && h.ts);
    // Prefer the mic (your voice); never act on system-audio output (other people on a call).
    const mic = hits.filter((h) => /input|microphone|mic/i.test(h.dev));
    const pool = mic.length ? mic : hits;
    const fresh = pool.filter((h) => h.ts > lastTs).sort((a, b) => (a.ts < b.ts ? -1 : 1));
    for (const h of fresh) {
      if (h.ts > lastTs) lastTs = h.ts;
      const cmd = await api.parseVoiceCommand(h.text).catch(() => null);
      if (cmd) dispatch(cmd);
    }
  } catch {
    /* recorder may be down / audio off — silently skip this tick */
  }
}

export function isVoiceEnabled(): boolean {
  return localStorage.getItem(KEY) === "1";
}

export function setVoiceEnabled(on: boolean) {
  localStorage.setItem(KEY, on ? "1" : "0");
  if (on && !timer) {
    lastTs = new Date().toISOString(); // only react to speech said AFTER enabling
    timer = setInterval(() => void tick(), 1800);
  } else if (!on && timer) {
    clearInterval(timer);
    timer = null;
  }
}

export function initVoice() {
  if (isVoiceEnabled()) setVoiceEnabled(true);
}
