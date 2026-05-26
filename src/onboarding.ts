import { $, wrap } from "./ui";
import { api } from "./api";

// First-run onboarding overlay: welcome → set up the recorder (permissions) → connect a model.
// Only shown on a genuine first run (recorder not installed). For an already-set-up Mac we mark
// it done immediately — re-running setup would re-sign the recorder and reset its permissions.

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

  if (localStorage.getItem(KEY) === "1") return;
  // Don't show (and never re-run setup) if the recorder is already installed — that's what was
  // resetting permissions. Mark onboarded so it stays out of the way.
  api
    .getState()
    .then((st) => {
      if (st.installed) {
        localStorage.setItem(KEY, "1");
        return;
      }
      show(0);
      $("onboard").classList.remove("hidden");
    })
    .catch(() => {});
}
