import { $, wrap } from "./ui";
import { api } from "./api";
import { goTab } from "./tabs";
import { showSettingsSub } from "./main";

// First-run onboarding overlay: welcome → set up the recorder (permissions) → connect a model.
// Shown once; "meye.onboarded" in localStorage marks completion.

const KEY = "meye.onboarded";
let step = 0;
const STEPS = 3;

function show(n: number) {
  step = Math.max(0, Math.min(STEPS - 1, n));
  document.querySelectorAll<HTMLElement>(".onboard-step").forEach((el) => {
    el.classList.toggle("hidden", Number(el.dataset.step) !== step);
  });
}

function finish() {
  localStorage.setItem(KEY, "1");
  $("onboard").classList.add("hidden");
}

export function initOnboarding() {
  $("onboard").querySelectorAll<HTMLElement>(".onboard-next").forEach((b) => (b.onclick = () => show(step + 1)));
  $("onboard").querySelectorAll<HTMLElement>(".onboard-back").forEach((b) => (b.onclick = () => show(step - 1)));
  $("onboard").querySelectorAll<HTMLElement>(".onboard-skip, .onboard-finish").forEach((b) => (b.onclick = finish));

  $("onboard-setup").onclick = async () => {
    $("onboard-setup-status").textContent = "Setting up… approve the macOS permission prompts.";
    await wrap("Set up & start", () => api.setup());
    $("onboard-setup-status").innerHTML =
      "✓ Recorder installed. If macOS asked for Screen Recording / Microphone, allow <b>Meye Recorder</b>, then continue.";
  };

  $("onboard-open-ai").onclick = () => {
    finish();
    goTab("settings");
    showSettingsSub("ai");
  };

  // Show on first run only.
  if (localStorage.getItem(KEY) !== "1") {
    show(0);
    $("onboard").classList.remove("hidden");
  }
}
