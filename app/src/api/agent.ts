// Bridge to the owner agent module running in the Tauri shell. It reports
// only what it truly detects on this machine; outside Tauri (vitest, plain
// browser dev) there is no agent module, so there are no suggestions.

import { inTauri } from "./tauri";
import type { ArtifactGroup, DraftOutcome, InsightsInput, LlmConfig, PathSnapshot, Preset } from "./types";

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

/** Bounded, redacted snapshot of shared files/directories, taken locally.
    null outside Tauri or when there is nothing to capture. */
export async function snapshotPaths(paths: string[]): Promise<PathSnapshot | null> {
  const wanted = [...new Set(paths.filter((p) => p.trim() !== ""))];
  if (!inTauri() || wanted.length === 0) {
    return null;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<PathSnapshot>("snapshot_artifacts", { paths: wanted });
  } catch (e) {
    console.error("artifact snapshot failed:", e);
    return null;
  }
}

async function invokeOrNull<T>(command: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!inTauri()) {
    return null;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<T>(command, args);
  } catch (e) {
    console.error(`${command} failed:`, e);
    return null;
  }
}

/** Owner: one JPEG frame of a granted window, or null on failure. */
export async function captureWindow(target: string): Promise<Uint8Array | null> {
  const b64 = await invokeOrNull<string>("capture_window", { target });
  if (!b64) {
    return null;
  }
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i += 1) {
    bytes[i] = raw.charCodeAt(i);
  }
  return bytes;
}

/** This machine's SSH public key; null when none exists (or outside Tauri). */
export async function sshPublicKey(): Promise<string | null> {
  return await invokeOrNull<string | null>("ssh_public_key");
}

/** Suggested user@host for granting SSH access to this machine. */
export async function sshTargetSuggestion(): Promise<string> {
  return (await invokeOrNull<string>("ssh_target_suggestion")) ?? "";
}

/** Owner: install a responder's public key into authorized_keys (tagged). */
export async function installSshKey(publicKey: string, marker: string): Promise<boolean> {
  if (!inTauri()) {
    return false;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("install_ssh_key", { publicKey, marker });
    return true;
  } catch (e) {
    console.error("install_ssh_key failed:", e);
    return false;
  }
}

/** Responder: open the system terminal running ssh to a granted target.
    Resolves to null on success, or a message saying why it failed. */
export async function openSsh(target: string): Promise<string | null> {
  if (!inTauri()) {
    return "not running inside the Cohort app";
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_ssh", { target });
    return null;
  } catch (e) {
    console.error("open_ssh failed:", e);
    return e instanceof Error ? e.message : String(e);
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

// ---- Assistant: the model runs from this machine, with this machine's
// settings. Nothing here reaches the hub.

export async function assistantPresets(): Promise<Preset[]> {
  return (await invokeOrNull<Preset[]>("assistant_presets")) ?? [];
}

/** This machine's assistant settings; null when none are saved. */
export async function assistantConfigGet(): Promise<LlmConfig | null> {
  return await invokeOrNull<LlmConfig | null>("assistant_config_get");
}

/** Save (or, with null, forget) this machine's assistant settings.
    Resolves to null on success, or a message saying why it failed. */
export async function assistantConfigSet(config: LlmConfig | null): Promise<string | null> {
  if (!inTauri()) {
    return "not running inside the Cohort app";
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("assistant_config_set", { config });
    return null;
  } catch (e) {
    console.error("assistant_config_set failed:", e);
    return e instanceof Error ? e.message : String(e);
  }
}

/** One tiny round trip with settings that may not be saved yet. */
export async function assistantConfigTest(config: LlmConfig): Promise<{ ok: boolean; message: string }> {
  if (!inTauri()) {
    return { ok: false, message: "not running inside the Cohort app" };
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const message = await invoke<string>("assistant_config_test", { config });
    return { ok: true, message };
  } catch (e) {
    return { ok: false, message: e instanceof Error ? e.message : String(e) };
  }
}

/** Draft the insights for a new assist on this machine. Outside Tauri there
    is no assistant: the draft is empty and the note says so. */
export async function draftInsights(input: InsightsInput): Promise<DraftOutcome> {
  const outcome = await invokeOrNull<DraftOutcome>("draft_insights", { input });
  return (
    outcome ?? {
      draft: { insights: "", environment: [] },
      note: "No assistant outside the Cohort app; insights left empty.",
      model: null,
      input_tokens: 0,
      output_tokens: 0,
    }
  );
}
