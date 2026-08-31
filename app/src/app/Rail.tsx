import { useState } from "react";
import { List, LogOut, Plus, Settings, User as UserIcon } from "lucide-react";
import { useApi } from "../api/hooks";
import { getHubUrl, setHubUrl } from "../api/hubUrl";
import type { User } from "../api/types";
import { AvatarChip, Modal } from "../components/ui";
import { useNav } from "./router";

export function Rail() {
  const { screen, navigate, currentUserId, signOut } = useNav();
  const { data: users } = useApi<User[]>("/api/users");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [urlDraft, setUrlDraft] = useState(getHubUrl());

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

      <button
        title="Settings"
        style={itemStyle(false)}
        onClick={() => {
          setUrlDraft(getHubUrl());
          setSettingsOpen(true);
        }}
      >
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
            Save
          </button>
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
