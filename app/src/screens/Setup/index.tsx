import { useCallback, useEffect, useState } from "react";
import { apiGet, apiPost } from "../../api/client";
import { getHubUrl, setHubUrl } from "../../api/hubUrl";
import type { User } from "../../api/types";
import { AvatarChip, Spinner } from "../../components/ui";
import { useNav } from "../../app/router";

/** First-launch identity: connect to the team hub, then register your name or
    pick your existing user. One identity per machine; sign-out lives in
    settings. */
export function Setup() {
  const { setUser } = useNav();
  const [hubDraft, setHubDraft] = useState(getHubUrl());
  const [users, setUsers] = useState<User[] | null>(null);
  const [hubError, setHubError] = useState<string | null>(null);
  const [checking, setChecking] = useState(true);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [registerError, setRegisterError] = useState<string | null>(null);

  const connect = useCallback(async (url: string) => {
    setChecking(true);
    setHubError(null);
    setUsers(null);
    setHubUrl(url);
    try {
      setUsers(await apiGet<User[]>("/api/users"));
    } catch (e) {
      setHubError(e instanceof Error ? e.message : String(e));
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    void connect(getHubUrl());
  }, [connect]);

  async function register() {
    setBusy(true);
    setRegisterError(null);
    try {
      const user = await apiPost<User>("/api/users", { name: name.trim() });
      setUser(user.id);
    } catch (e) {
      setRegisterError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  return (
    <div style={{ display: "grid", placeItems: "center", minHeight: "100vh" }}>
      <div className="card fade-in" style={{ width: 460, padding: 28 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 6 }}>
          <span
            style={{
              width: 30,
              height: 30,
              borderRadius: 8,
              background: "var(--color-accent)",
              display: "grid",
              placeItems: "center",
            }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2">
              <circle cx="12" cy="12" r="3" />
              <circle cx="12" cy="12" r="8" opacity="0.6" />
            </svg>
          </span>
          <h2 style={{ fontSize: 19 }}>Cohort</h2>
        </div>
        <p style={{ fontSize: 13, color: "var(--color-neutral-600)", margin: "0 0 16px" }}>
          Connect to your team's hub, then say who you are. This machine keeps
          that identity.
        </p>

        <div className="field" style={{ marginBottom: 12 }}>
          <label>Hub URL</label>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input mono"
              value={hubDraft}
              onChange={(e) => setHubDraft(e.target.value)}
              placeholder="http://hub.internal:7400"
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  void connect(hubDraft);
                }
              }}
            />
            <button className="btn" onClick={() => void connect(hubDraft)}>
              Connect
            </button>
          </div>
        </div>

        {checking && (
          <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
            <Spinner size={14} /> Reaching the hub...
          </span>
        )}
        {hubError && (
          <div style={{ fontSize: 12.5, color: "var(--color-accent-700)", marginBottom: 10 }}>
            Could not reach the hub: {hubError}
          </div>
        )}

        {users && (
          <>
            <div className="field" style={{ marginBottom: 12 }}>
              <label>I am new here</label>
              <div style={{ display: "flex", gap: 8 }}>
                <input
                  className="input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Your name"
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && name.trim()) {
                      void register();
                    }
                  }}
                />
                <button
                  className="btn btn-primary"
                  disabled={busy || !name.trim()}
                  onClick={() => void register()}
                >
                  Join
                </button>
              </div>
              {registerError && (
                <span style={{ fontSize: 12, color: "var(--color-accent-700)" }}>{registerError}</span>
              )}
            </div>

            {users.length > 0 && (
              <div className="field">
                <label>Or continue as an existing user</label>
                <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 220, overflowY: "auto" }}>
                  {users.map((u) => (
                    <button
                      key={u.id}
                      className="btn"
                      style={{ justifyContent: "flex-start", gap: 10 }}
                      onClick={() => setUser(u.id)}
                    >
                      <AvatarChip name={u.name} size={24} />
                      {u.name}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
