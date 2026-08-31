// Bridge to the owner agent module running in the Tauri shell. Outside Tauri
// (vitest, plain browser dev) a fixture mirrors the stub agent so the picker
// still works.

import type { ArtifactGroup } from "./types";

const FIXTURE: ArtifactGroup[] = [
  {
    title: "Terminals",
    items: [
      { id: "t1", kind: "terminal", badge: "iT", label: "iTerm2 (payments)", detail: "last command: kubectl rollout status", warn: false },
      { id: "t2", kind: "terminal", badge: "VS", label: "VS Code (zsh)", detail: "integrated terminal, 2 tabs", warn: false },
      { id: "t3", kind: "terminal", badge: ">_", label: "Terminal (ssh)", detail: "ssh staging-02 - idle 18m", warn: true },
    ],
  },
  {
    title: "Files",
    items: [
      { id: "f1", kind: "file", badge: "YML", label: "deployment.yaml", detail: "k8s/payments - ref a3f9c1", warn: false },
      { id: "f2", kind: "file", badge: "YML", label: "kustomization.yaml", detail: "k8s/payments - ref a3f9c1", warn: false },
      { id: "f3", kind: "file", badge: "YML", label: "values.yaml", detail: "charts/payments - ref a3f9c1", warn: true },
    ],
  },
  {
    title: "AI agents",
    items: [
      { id: "a1", kind: "ai_agent", badge: "CC", label: "Claude Code", detail: "agent session active - 41 turns", warn: false },
      { id: "a2", kind: "ai_agent", badge: "Cu", label: "Cursor", detail: "agent session idle - 12 turns", warn: false },
    ],
  },
];

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function suggestArtifacts(): Promise<ArtifactGroup[]> {
  if (inTauri()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke<ArtifactGroup[]>("suggest_artifacts");
    } catch {
      return FIXTURE;
    }
  }
  return FIXTURE;
}

export async function envFingerprint(): Promise<string[]> {
  if (inTauri()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke<string[]>("env_fingerprint");
    } catch {
      // fall through to fixture
    }
  }
  return ["Kubernetes 1.29", "Helm 3.14", "registry.internal:5000", "Linux amd64"];
}
