import { useState } from "react";
import { List, LogOut, Plus, Settings, User as UserIcon } from "lucide-react";
import {
  assistantConfigGet,
  assistantConfigSet,
  assistantConfigTest,
  assistantPresets,
} from "../api/agent";
import { useApi } from "../api/hooks";
import { getHubUrl, setHubUrl } from "../api/hubUrl";
import type { LlmConfig, Preset, User } from "../api/types";
import { AvatarChip, Modal } from "../components/ui";
import { useNav } from "./router";

const EMPTY_ASSISTANT: LlmConfig = {
  protocol: "openai_compatible",
  base_url: "",
  api_key: null,
  model: "",
};

export function Rail() {
  const { screen, navigate, currentUserId, signOut } = useNav();
  const { data: users } = useApi<User[]>("/api/users");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [urlDraft, setUrlDraft] = useState(getHubUrl());

  // Assistant settings: the model this machine drafts insights with.
  const [presets, setPresets] = useState<Preset[]>([]);
  const [presetId, setPresetId] = useState("");
  const [assistant, setAssistant] = useState<LlmConfig>(EMPTY_ASSISTANT);
  const [assistantSaved, setAssistantSaved] = useState(false);
  const [assistantBusy, setAssistantBusy] = useState(false);
  const [assistantNote, setAssistantNote] = useState<{ ok: boolean; text: string } | null>(null);
  const canSaveAssistant = assistant.base_url.trim() !== "" && assistant.model.trim() !== "";

  async function openSettings() {
    setUrlDraft(getHubUrl());
    setAssistantNote(null);
    const [list, current] = await Promise.all([assistantPresets(), assistantConfigGet()]);
    setPresets(list);
    setAssistantSaved(current !== null);
    setAssistant(current ?? EMPTY_ASSISTANT);
    setPresetId(
      current ? (list.find((p) => p.base_url === current.base_url && p.protocol === current.protocol)?.id ?? "") : "",
    );
    setSettingsOpen(true);
  }

  function applyPreset(id: string) {
    setPresetId(id);
    const preset = presets.find((p) => p.id === id);
    if (preset) {
      setAssistant((a) => ({
        ...a,
        protocol: preset.protocol,
        base_url: preset.base_url,
        model: preset.default_model || a.model,
      }));
    }
  }

  function assistantToSave(): LlmConfig {
    const key = assistant.api_key?.trim() ?? "";
    return {
      protocol: assistant.protocol,
      base_url: assistant.base_url.trim(),
      model: assistant.model.trim(),
      api_key: key === "" ? null : key,
    };
  }

  async function testAssistant() {
    setAssistantBusy(true);
    setAssistantNote(null);
    const result = await assistantConfigTest(assistantToSave());
    setAssistantNote({ ok: result.ok, text: result.message });
    setAssistantBusy(false);
  }

  async function saveAssistant() {
    setAssistantBusy(true);
    setAssistantNote(null);
    const error = await assistantConfigSet(assistantToSave());
    setAssistantNote(error ? { ok: false, text: error } : { ok: true, text: "Saved. Insights are drafted on this machine." });
    setAssistantSaved(error === null);
    setAssistantBusy(false);
  }

  async function removeAssistant() {
    setAssistantBusy(true);
    setAssistantNote(null);
    const error = await assistantConfigSet(null);
    if (error === null) {
      setAssistant(EMPTY_ASSISTANT);
      setPresetId("");
      setAssistantSaved(false);
      setAssistantNote({ ok: true, text: "Removed. Insights stay empty until an assistant is configured." });
    } else {
      setAssistantNote({ ok: false, text: error });
    }
    setAssistantBusy(false);
  }

  const onAssists = ["assists", "assist", "close"].includes(screen.name);
  const onRecord = screen.name === "record";
  const currentUser = users?.find((u) => u.id === currentUserId);

  const itemStyle = (active: boolean) => ({
    display: "grid" as const,
    placeItems: "center" as const,
    width: 40,
    height: 40,
    borderRadius: 10,
    cursor: "pointer",
    border: "none",
    background: active ? "var(--color-accent-100)" : "transparent",
    color: active ? "var(--color-accent-700)" : "var(--color-neutral-600)",
  });

  return (
    <aside
      style={{
        width: 64,
        background: "#fff",
        boxShadow: "var(--shadow-sm)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 10,
        padding: "14px 0",
        position: "sticky",
        top: 0,
        height: "100vh",
      }}
    >
      <div
        title="Cohort: agent connected, outbound only"
        style={{
          width: 36,
          height: 36,
          borderRadius: 9,
          background: "var(--color-accent)",
          display: "grid",
          placeItems: "center",
          marginBottom: 6,
        }}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2">
          <circle cx="12" cy="12" r="3" />
          <circle cx="12" cy="12" r="8" opacity="0.6" />
        </svg>
      </div>

      <button
        title="Open assists"
        style={itemStyle(onAssists)}
        onClick={() => navigate({ name: "assists" })}
      >
        <List size={18} />
      </button>
      <button
        title="My record, private to you"
        style={itemStyle(onRecord)}
        onClick={() => navigate({ name: "record" })}
      >
        <UserIcon size={18} />
      </button>

      <div style={{ flex: 1 }} />

      <button
        title="Open an assist"
        onClick={() => navigate({ name: "new" })}
        style={{
          width: 40,
          height: 40,
          borderRadius: 10,
          border: "none",
          background: "var(--color-accent)",
          color: "#fff",
          display: "grid",
          placeItems: "center",
          cursor: "pointer",
          boxShadow: "var(--shadow-sm)",
        }}
      >
        <Plus size={20} />
      </button>

      <button title="Settings" style={itemStyle(false)} onClick={() => void openSettings()}>
        <Settings size={17} />
      </button>

      <span title={currentUser ? `Signed in as ${currentUser.name}` : "Signed in"}>
        <AvatarChip name={currentUser?.name ?? "?"} active size={30} />
      </span>

      {settingsOpen && (
        <Modal width={420} onClose={() => setSettingsOpen(false)}>
          <h3 style={{ fontSize: 16, marginBottom: 12 }}>Settings</h3>
          <div className="field">
            <label>Hub URL</label>
            <input
              className="input mono"
              value={urlDraft}
              onChange={(e) => setUrlDraft(e.target.value)}
              placeholder="http://hub.internal:7400"
            />
          </div>
          <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: "10px 0 14px" }}>
            The address of your team's cohort-hub, e.g. a server in your company's
            data centre. Reloads the app on save.
          </p>
          <button
            className="btn btn-primary btn-block"
            style={{ marginBottom: 10 }}
            onClick={() => {
              setHubUrl(urlDraft);
              setSettingsOpen(false);
              window.location.reload();
            }}
          >
            Save hub URL
          </button>

          <hr style={{ border: 0, borderTop: "1px solid var(--color-neutral-200)", margin: "16px 0" }} />
          <h3 style={{ fontSize: 16, marginBottom: 4 }}>Assistant</h3>
          <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: "0 0 12px" }}>
            Drafts the insights when you open an assist, running from this machine
            with the provider below. Your description and the files you chose to
            share go to that provider and nowhere else; a local server keeps them
            on this machine.
          </p>
          <div className="field">
            <label htmlFor="assistant-preset">Provider</label>
            <select
              id="assistant-preset"
              className="input"
              value={presetId}
              onChange={(e) => applyPreset(e.target.value)}
            >
              <option value="">Choose a preset</option>
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label htmlFor="assistant-base-url">Base URL</label>
            <input
              id="assistant-base-url"
              className="input mono"
              value={assistant.base_url}
              onChange={(e) => setAssistant((a) => ({ ...a, base_url: e.target.value }))}
              placeholder="http://localhost:11434/v1"
            />
          </div>
          <div className="field">
            <label htmlFor="assistant-model">Model</label>
            <input
              id="assistant-model"
              className="input mono"
              value={assistant.model}
              onChange={(e) => setAssistant((a) => ({ ...a, model: e.target.value }))}
              placeholder="model id from your provider"
            />
          </div>
          <div className="field">
            <label htmlFor="assistant-api-key">API key</label>
            <input
              id="assistant-api-key"
              className="input mono"
              type="password"
              value={assistant.api_key ?? ""}
              onChange={(e) => setAssistant((a) => ({ ...a, api_key: e.target.value }))}
              placeholder={
                presets.find((p) => p.id === presetId)?.needs_key === false
                  ? "not needed for this provider"
                  : "required by this provider"
              }
            />
          </div>
          {assistantNote && (
            <div
              style={{
                fontSize: 12.5,
                color: assistantNote.ok ? "var(--color-success-fg)" : "var(--color-accent-700)",
                margin: "0 0 10px",
              }}
            >
              {assistantNote.text}
            </div>
          )}
          <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
            <button className="btn" disabled={assistantBusy || !canSaveAssistant} onClick={() => void testAssistant()}>
              Test
            </button>
            <button
              className="btn btn-primary"
              disabled={assistantBusy || !canSaveAssistant}
              onClick={() => void saveAssistant()}
            >
              Save assistant
            </button>
            {assistantSaved && (
              <button className="btn" disabled={assistantBusy} onClick={() => void removeAssistant()}>
                Remove
              </button>
            )}
          </div>

          <hr style={{ border: 0, borderTop: "1px solid var(--color-neutral-200)", margin: "0 0 12px" }} />
          <button
            className="btn btn-block"
            onClick={() => {
              setSettingsOpen(false);
              signOut();
            }}
          >
            <LogOut size={13} />
            Sign out{currentUser ? ` (${currentUser.name})` : ""}
          </button>
        </Modal>
      )}
    </aside>
  );
}
