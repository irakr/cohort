/** True when running inside the Tauri desktop shell (not vitest/browser). */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
