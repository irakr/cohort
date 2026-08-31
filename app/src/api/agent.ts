// Bridge to the owner agent module running in the Tauri shell. It reports
// only what it truly detects on this machine; outside Tauri (vitest, plain
// browser dev) there is no agent module, so there are no suggestions.

import type { ArtifactGroup } from "./types";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function suggestArtifacts(): Promise<ArtifactGroup[]> {
  if (!inTauri()) {
    return [];
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<ArtifactGroup[]>("suggest_artifacts");
  } catch {
    return [];
  }
}

export async function envFingerprint(): Promise<string[]> {
  if (!inTauri()) {
    return [];
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<string[]>("env_fingerprint");
  } catch {
    return [];
  }
}
