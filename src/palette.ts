import { $ } from "./ui";
import { goTab, type Tab } from "./tabs";

type Action = { label: string; run: () => void };

const TAB_ACTIONS: { tab: Tab; label: string }[] = [
  { tab: "home", label: "Go to Home" },
  { tab: "status", label: "Go to Status" },
  { tab: "search", label: "Go to Search" },
  { tab: "timeline", label: "Go to Timeline" },
  { tab: "devices", label: "Go to Devices" },
  { tab: "pipes", label: "Go to Pipes" },
  { tab: "settings", label: "Go to Settings" },
  { tab: "advanced", label: "Go to Advanced" },
];

let filtered: Action[] = [];
let sel = 0;

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch]!));
}

function build(q: string) {
  const query = q.trim();
  const lc = query.toLowerCase();
  const tabActs: Action[] = TAB_ACTIONS.filter((t) => !lc || t.label.toLowerCase().includes(lc)).map((t) => ({
    label: t.label,
    run: () => {
      close();
      goTab(t.tab);
    },
  }));
  const extra: Action[] = query
    ? [
        {
          label: `Search recordings for “${query}”`,
          run: () => {
            close();
            goTab("search");
            ($("s-q") as HTMLInputElement).value = query;
            ($("s-go") as HTMLButtonElement).click();
          },
        },
        {
          label: `Ask Meye: “${query}”`,
          run: () => {
            close();
            goTab("home");
            ($("home-input") as HTMLTextAreaElement).value = query;
            ($("home-send") as HTMLButtonElement).click();
          },
        },
      ]
    : [];
  filtered = [...extra, ...tabActs];
  sel = 0;
  render();
}

function render() {
  const list = $("pal-list");
  list.innerHTML = "";
  filtered.forEach((a, i) => {
    const item = document.createElement("div");
    item.className = "pal-item" + (i === sel ? " active" : "");
    item.innerHTML = esc(a.label);
    item.onmouseenter = () => {
      sel = i;
      render();
    };
    item.onclick = () => a.run();
    list.appendChild(item);
  });
}

function open() {
  $("palette").classList.remove("hidden");
  const i = $("pal-input") as HTMLInputElement;
  i.value = "";
  build("");
  i.focus();
}

function close() {
  $("palette").classList.add("hidden");
}

function isOpen(): boolean {
  return !$("palette").classList.contains("hidden");
}

export function initPalette() {
  document.addEventListener("keydown", (e) => {
    const ke = e as KeyboardEvent;
    if ((ke.metaKey || ke.ctrlKey) && ke.key.toLowerCase() === "k") {
      e.preventDefault();
      isOpen() ? close() : open();
    } else if (ke.key === "Escape" && isOpen()) {
      close();
    }
  });
  ($("pal-input") as HTMLInputElement).addEventListener("input", (e) => build((e.target as HTMLInputElement).value));
  ($("pal-input") as HTMLInputElement).addEventListener("keydown", (e) => {
    const ke = e as KeyboardEvent;
    if (ke.key === "ArrowDown") {
      e.preventDefault();
      sel = Math.min(filtered.length - 1, sel + 1);
      render();
    } else if (ke.key === "ArrowUp") {
      e.preventDefault();
      sel = Math.max(0, sel - 1);
      render();
    } else if (ke.key === "Enter") {
      e.preventDefault();
      filtered[sel]?.run();
    }
  });
  $("palette").addEventListener("click", (e) => {
    if (e.target === $("palette")) close();
  });
}
