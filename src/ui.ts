export const $ = (id: string) => document.getElementById(id)!;

let toastTimer: ReturnType<typeof setTimeout> | null = null;

/** Show a toast. `sticky` keeps it until the next toast; `spin` prefixes a spinner. */
export function toast(msg: string, opts: { sticky?: boolean; spin?: boolean } = {}) {
  const t = $("toast");
  if (toastTimer) {
    clearTimeout(toastTimer);
    toastTimer = null;
  }
  t.classList.toggle("with-spin", !!opts.spin);
  t.textContent = msg;
  t.classList.remove("hidden");
  if (!opts.sticky) toastTimer = setTimeout(() => t.classList.add("hidden"), 4000);
}

/** Run an async action with visible progress: a spinner toast while it runs,
 * then a ✓ / failure toast. Used by every button that calls the backend. */
export async function wrap(label: string, fn: () => Promise<unknown>) {
  toast(`${label}…`, { sticky: true, spin: true });
  try {
    await fn();
    toast(`${label} ✓`);
  } catch (e) {
    toast(`${label} failed: ${e}`);
  }
}
