import { useState } from "react";
import { List, Plus, Settings, User as UserIcon } from "lucide-react";
import { useApi } from "../api/hooks";
import { getHubUrl, setHubUrl } from "../api/hubUrl";
import type { User } from "../api/types";
import { AvatarChip, Modal } from "../components/ui";
import { useNav } from "./router";

export function Rail() {
  const { screen, navigate, currentUserId, setUser } = useNav();
  const { data: users } = useApi<User[]>("/api/users");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [switcherOpen, setSwitcherOpen] = useState(false);
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
        title="Hub settings"
        style={itemStyle(false)}
        onClick={() => {
          setUrlDraft(getHubUrl());
          setSettingsOpen(true);
        }}
      >
        <Settings size={17} />
      </button>

      <button
        title={currentUser ? `Acting as ${currentUser.name}` : "Switch user"}
        onClick={() => setSwitcherOpen(true)}
        style={{ border: "none", background: "transparent", cursor: "pointer", padding: 0 }}
      >
        <AvatarChip name={currentUser?.name ?? "?"} active size={30} />
      </button>

      {settingsOpen && (
        <Modal width={420} onClose={() => setSettingsOpen(false)}>
          <h3 style={{ fontSize: 16, marginBottom: 12 }}>Hub connection</h3>
          <div className="field">
            <label>Hub URL</label>
            <input
              className="input mono"
              value={urlDraft}
              onChange={(e) => setUrlDraft(e.target.value)}
              placeholder="http://127.0.0.1:7400"
            />
          </div>
          <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: "10px 0 14px" }}>
            The address of your team's cohort-hub, e.g. a server in your company's
            data centre. Reloads the app on save.
          </p>
          <button
            className="btn btn-primary btn-block"
            onClick={() => {
              setHubUrl(urlDraft);
              setSettingsOpen(false);
              window.location.reload();
            }}
          >
            Save
          </button>
        </Modal>
      )}

      {switcherOpen && users && (
        <Modal width={320} onClose={() => setSwitcherOpen(false)}>
          <h3 style={{ fontSize: 16, marginBottom: 4 }}>Act as</h3>
          <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: "0 0 12px" }}>
            Seeded users, for walking owner and responder flows.
          </p>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {users.map((u) => (
              <button
                key={u.id}
                className="btn"
                style={{ justifyContent: "flex-start", gap: 10 }}
                onClick={() => {
                  setUser(u.id);
                  setSwitcherOpen(false);
                  navigate({ name: "assists" });
                }}
              >
                <AvatarChip name={u.name} active={u.id === currentUserId} size={24} />
                {u.name}
                {u.id === currentUserId && (
                  <span style={{ marginLeft: "auto", fontSize: 11, color: "var(--color-neutral-500)" }}>
                    current
                  </span>
                )}
              </button>
            ))}
          </div>
        </Modal>
      )}
    </aside>
  );
}
