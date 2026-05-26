import { api } from "./api";
import { toast } from "./ui";
import { goTab, type Tab } from "./tabs";
import { startNewChat } from "./home";
import { setVisionPaused } from "./performance";
import { runSearchWith } from "./search";

// Push-to-talk voice commands. Instead of polling the transcript continuously (heavy, and it
// kept the recorder in realtime mode), you tap the floating mic button and speak one command.
// We then watch the local mic transcript for a few seconds, parse it (Rust), and act — fully
// local, and only working when you ask it to.

const KEY = "meye.voice"; // controls whether the mic button is shown
let listening = false;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

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
    case "open": {
      // status/devices/performance are now sub-tabs of Settings; map them so we don't open a tab that no longer exists.
      const map: Record<string, Tab> = { status: "settings", devices: "settings", performance: "settings", chat: "home" };
      goTab((map[cmd.arg] ?? cmd.arg) as Tab);
      toast(`🎙 Opened ${cmd.arg}`);
      break;
    }
    case "search":
      goTab("search");
      runSearchWith(cmd.arg);
      toast(`🎙 Searching "${cmd.arg}"`);
      break;
  }
}

/** Listen for one spoken command for up to ~8s, then act. Reads the local mic transcript. */
export async function pushToTalk() {
  if (listening) return;
  listening = true;
  const fab = document.getElementById("voice-fab");
  fab?.classList.add("listening");
  toast("🎙 Listening… say a command", { sticky: true, spin: true });

  const since = new Date().toISOString();
  const deadline = Date.now() + 8000;
  let acted = false;
  while (Date.now() < deadline && !acted) {
    await sleep(700);
    const res: any = await api.search({ content_type: "audio", limit: 5 }).catch(() => null);
    if (!res) continue;
    const hits = ((res.data ?? []) as any[])
      .map((h) => {
        const c = h.content ?? h;
        return { ts: String(c.timestamp ?? ""), text: String(c.transcription ?? c.text ?? ""), dev: String(c.device_name ?? "") };
      })
      .filter((h) => h.text && h.ts >= since && /input|microphone|mic/i.test(h.dev))
      .sort((a, b) => (a.ts < b.ts ? -1 : 1));
    for (const h of hits) {
      const cmd = await api.parseVoiceCommand(h.text).catch(() => null);
      if (cmd) {
        dispatch(cmd);
        acted = true;
        break;
      }
    }
  }

  fab?.classList.remove("listening");
  listening = false;
  if (!acted) {
    toast('Didn\'t catch a command. Try "pause", "new chat", "open timeline", "search for …". (Audio must be on.)');
  }
}

export function isVoiceEnabled(): boolean {
  return localStorage.getItem(KEY) === "1";
}

export function setVoiceEnabled(on: boolean) {
  localStorage.setItem(KEY, on ? "1" : "0");
  document.getElementById("voice-fab")?.classList.toggle("hidden", !on);
}

export function initVoice() {
  const fab = document.getElementById("voice-fab");
  if (fab) fab.onclick = () => void pushToTalk();
  setVoiceEnabled(isVoiceEnabled());
}
