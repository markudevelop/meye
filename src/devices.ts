import { $, wrap } from "./ui";
import { api } from "./api";

async function refresh() {
  try {
    $("d-monitors").textContent = JSON.stringify(await api.monitors(), null, 2);
  } catch (e) {
    $("d-monitors").textContent = `Failed: ${e} (recorder running?)`;
  }
  try {
    $("d-audio").textContent = JSON.stringify(await api.audioDevices(), null, 2);
  } catch (e) {
    $("d-audio").textContent = `Failed: ${e} (recorder running?)`;
  }
}

export function initDevices() {
  $("d-refresh").onclick = () => void refresh();
  $("d-audio-start").onclick = () => wrap("Start audio", () => api.audioStart()).then(refresh);
  $("d-audio-stop").onclick = () => wrap("Stop audio", () => api.audioStop()).then(refresh);
}
