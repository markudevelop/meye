export const $ = (id: string) => document.getElementById(id)!;

export function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  setTimeout(() => t.classList.add("hidden"), 4000);
}

export async function wrap(label: string, fn: () => Promise<unknown>) {
  try {
    await fn();
    toast(`${label} ✓`);
  } catch (e) {
    toast(`${label} failed: ${e}`);
  }
}
