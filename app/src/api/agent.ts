// Bridge to the owner agent module running in the Tauri shell. It reports
// only what it truly detects on this machine; outside Tauri (vitest, plain
// browser dev) there is no agent module, so there are no suggestions.

import { inTauri } from "./tauri";
import type { ArtifactGroup, PathSnapshot } from "./types";

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

/** Owner: process activity in the granted terminals (feed lines per tty). */
export async function terminalActivity(labels: string[]): Promise<string[] | null> {
  if (labels.length === 0) {
    return null;
  }
  return await invokeOrNull<string[]>("terminal_activity", { labels });
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

/** Responder: open the system terminal running ssh to a granted target. */
export async function openSsh(target: string): Promise<boolean> {
  if (!inTauri()) {
    return false;
  }
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_ssh", { target });
    return true;
  } catch (e) {
    console.error("open_ssh failed:", e);
    return false;
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
