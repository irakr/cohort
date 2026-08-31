import { useState } from "react";
import {
  ArrowLeft,
  Bot,
  Check,
  ChevronLeft,
  Clock,
  FileText,
  Lock,
  MessageCircle,
  Radio,
  SquareTerminal,
  TerminalSquare,
  User as UserIcon,
  Users,
  X,
} from "lucide-react";
import { apiPost } from "../../api/client";
import { useApi } from "../../api/hooks";
import type {
  AssistDetail as AssistDetailT,
  Grant,
  LiveData,
  ScopeKind,
  ScopeRequest,
} from "../../api/types";
import { FileTree } from "../../components/FileTree";
import { TerminalPane } from "../../components/TerminalPane";
import { IconTile, Modal, SectionTitle, Spinner, StatusPill } from "../../components/ui";
import { CATEGORY_LABELS, renderMarkdown, timeAgo } from "../../util";
import { useNav } from "../../app/router";
import { getCurrentUserId } from "../../api/currentUser";

export function AssistDetail({ assistRef }: { assistRef: string }) {
  const { navigate } = useNav();
  const me = getCurrentUserId();
  const {
    data: assist,
    loading,
    error,
    refetch,
  } = useApi<AssistDetailT>(`/api/assists/${assistRef}`, { pollMs: 5000 });
  const isMember = !!assist && (assist.viewer_is_owner || assist.viewer_is_responder);
  const { data: liveData } = useApi<LiveData>(
    isMember ? `/api/assists/${assistRef}/artifacts` : null,
  );
  const [viewFile, setViewFile] = useState<string | null>(null);
  const [requestOpen, setRequestOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  if (loading) {
    return (
      <div style={{ display: "grid", placeItems: "center", minHeight: "60vh" }}>
        <Spinner size={22} />
      </div>
    );
  }
  if (error || !assist) {
    return (
      <div style={{ maxWidth: 860, margin: "0 auto", padding: "34px 28px" }}>
        <div className="card" style={{ padding: 18, color: "var(--color-accent-700)" }}>
          {error ?? "Assist not found"}
        </div>
      </div>
    );
  }

  const myGrants = assist.grants.filter((g) => g.granted_to_id === me);
  const grantFor = (kind: ScopeKind): Grant | undefined => myGrants.find((g) => g.kind === kind);
  const myPending = (kind: ScopeKind): ScopeRequest | undefined =>
    assist.scope_requests.find(
      (r) => r.kind === kind && r.requester_id === me && r.status === "pending",
    );
  const isLive = !!grantFor("live_debug");

  async function act(fn: () => Promise<unknown>) {
    setBusy(true);
    try {
      await fn();
      refetch();
    } catch (e) {
      alert(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ maxWidth: assist.viewer_is_responder && isLive ? 1200 : 860, margin: "0 auto", padding: "30px 28px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 6 }}>
        <button
          className="btn"
          style={{ padding: 7 }}
          title="Back to assists"
          onClick={() => navigate({ name: "assists" })}
        >
          <ArrowLeft size={15} />
        </button>
        <span style={{ fontSize: 12, fontWeight: 700, color: "var(--color-neutral-600)" }}>
          {assist.ref}
        </span>
        <StatusPill status={assist.status} />
        {assist.category && <span className="tag tag-neutral">{CATEGORY_LABELS[assist.category]}</span>}
      </div>
      <h1 style={{ fontSize: 21, fontWeight: 700, maxWidth: "48ch", marginBottom: 10 }}>
        {assist.title}
      </h1>
      <div
        style={{
          display: "flex",
          gap: 18,
          fontSize: 12.5,
          color: "var(--color-neutral-600)",
          marginBottom: 24,
          flexWrap: "wrap",
        }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <UserIcon size={13} /> {assist.owner_name}
        </span>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Clock size={13} /> opened {timeAgo(assist.created_at)} ago
        </span>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Users size={13} />
          {assist.responders.length > 0
            ? `Responding: ${assist.responders.map((r) => r.name).join(", ")}`
            : "no responder yet"}
        </span>
        <span style={{ display: "flex", gap: 6 }}>
          {assist.tags.map((t) => (
            <span key={t} className="tag tag-neutral">
              {t}
            </span>
          ))}
        </span>
      </div>

      <Brief assist={assist} />

      {assist.viewer_is_owner && assist.status !== "done" && (
        <OwnerPanel assist={assist} busy={busy} act={act} onClose={() => navigate({ name: "close", ref: assist.ref })} />
      )}

      {!assist.viewer_is_owner && assist.status !== "done" && (
        <ResponderPanel
          assist={assist}
          liveData={liveData}
          isLive={isLive}
          grantFor={grantFor}
          myPending={myPending}
          busy={busy}
          act={act}
          onOpenFile={setViewFile}
          onRequestArtifacts={() => setRequestOpen(true)}
        />
      )}

      {assist.status === "done" && (
        <div className="card" style={{ padding: 18, fontSize: 13.5, color: "var(--color-neutral-600)" }}>
          This assist is closed. Its resolution record is kept indefinitely.
        </div>
      )}

      {viewFile && liveData && (
        <Modal width={660} onClose={() => setViewFile(null)}>
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
            <IconTile bg="var(--color-success-bg)" fg="var(--color-success)">
              {viewFile.endsWith(".log") ? "LOG" : viewFile.endsWith(".rs") ? "RS" : viewFile.endsWith(".sql") ? "SQL" : "YML"}
            </IconTile>
            <span style={{ fontWeight: 600, fontSize: 14 }}>{viewFile}</span>
            <span className="tag tag-neutral mono">ref a3f9c1</span>
          </div>
          <pre
            className="mono"
            style={{
              background: "var(--color-neutral-900)",
              color: "var(--color-neutral-200)",
              borderRadius: 8,
              padding: 14,
              fontSize: 12,
              whiteSpace: "pre-wrap",
              margin: 0,
            }}
          >
            {liveData.files[viewFile] ?? "not granted"}
          </pre>
        </Modal>
      )}

      {requestOpen && (
        <RequestArtifactsModal
          assist={assist}
          onClose={() => setRequestOpen(false)}
          onSend={(kind, target, reason) =>
            void act(async () => {
              await apiPost(`/api/assists/${assist.ref}/scope-requests`, {
                kind,
                target,
                reason,
                ttl_minutes: 240,
              });
              setRequestOpen(false);
            })
          }
        />
      )}
    </div>
  );
}

function Brief({ assist }: { assist: AssistDetailT }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, marginBottom: 26 }}>
      <section>
        <SectionTitle>Goal</SectionTitle>
        <div
          style={{ fontSize: 14, lineHeight: 1.6 }}
          dangerouslySetInnerHTML={{ __html: renderMarkdown(assist.goal) }}
        />
      </section>
      {assist.failures.length > 0 && (
        <section>
          <SectionTitle>Failures</SectionTitle>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {assist.failures.map((f, i) => (
              <div
                key={i}
                className="card"
                style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px" }}
              >
                <span className="mono" style={{ fontSize: 12.5, flex: 1 }}>
                  {f.label}
                </span>
                <span className="tag tag-neutral">{f.note}</span>
              </div>
            ))}
          </div>
        </section>
      )}
      {assist.environment.length > 0 && (
        <section>
          <SectionTitle>Environment</SectionTitle>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            {assist.environment.map((e) => (
              <span key={e} className="tag tag-neutral mono" style={{ fontSize: 11 }}>
                {e}
              </span>
            ))}
          </div>
        </section>
      )}
      {assist.artifacts.length > 0 && (
        <section>
          <SectionTitle>Shared at open</SectionTitle>
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
            {assist.artifacts.map((a) => (
              <span key={a.id} className="tag tag-accent" title={a.detail}>
                {a.kind === "terminal" ? <SquareTerminal size={11} /> : a.kind === "ai_agent" ? <Bot size={11} /> : <FileText size={11} />}
                {a.label}
              </span>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

const KIND_ICON: Record<ScopeKind, typeof FileText> = {
  comment: MessageCircle,
  live_debug: Radio,
  file: FileText,
  terminal: SquareTerminal,
  agents: Bot,
  ssh: TerminalSquare,
};

function requestTitle(r: ScopeRequest): string {
  if (r.kind === "comment") {
    return r.reason;
  }
  if (r.kind === "live_debug") {
    return "Live debug request";
  }
  return r.target ?? r.kind;
}

function OwnerPanel({
  assist,
  busy,
  act,
  onClose,
}: {
  assist: AssistDetailT;
  busy: boolean;
  act: (fn: () => Promise<unknown>) => Promise<void>;
  onClose: () => void;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <section>
        <SectionTitle>Responds</SectionTitle>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {assist.scope_requests.length === 0 && (
            <div className="card" style={{ padding: 16, fontSize: 13, color: "var(--color-neutral-600)" }}>
              Nothing yet. Requests from responders land here for one-tap approval.
            </div>
          )}
          {assist.scope_requests.map((r) => {
            const Icon = KIND_ICON[r.kind];
            const isComment = r.kind === "comment";
            return (
              <div
                key={r.id}
                className="card"
                style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 14px" }}
              >
                <IconTile
                  size={26}
                  bg={isComment ? "var(--color-neutral-100)" : "var(--color-accent-100)"}
                  fg={isComment ? "var(--color-neutral-700)" : "var(--color-accent-700)"}
                >
                  <Icon size={13} />
                </IconTile>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13.5, fontWeight: isComment ? 400 : 600 }}>
                    {requestTitle(r)}
                  </div>
                  <div style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>
                    {isComment ? r.requester_name : `${r.requester_name}: "${r.reason}"`}
                  </div>
                </div>
                {r.status === "pending" ? (
                  <>
                    <button
                      className="btn"
                      style={{ width: 32, height: 32, padding: 0 }}
                      title="Deny"
                      disabled={busy}
                      onClick={() => void act(() => apiPost(`/api/scope-requests/${r.id}/deny`))}
                    >
                      <X size={14} />
                    </button>
                    <button
                      className="btn btn-primary"
                      style={{ width: 32, height: 32, padding: 0 }}
                      title={r.kind === "live_debug" ? "Accept and go live" : "Approve for 4h"}
                      disabled={busy}
                      onClick={() => void act(() => apiPost(`/api/scope-requests/${r.id}/approve`))}
                    >
                      <Check size={14} />
                    </button>
                  </>
                ) : (
                  !isComment && (
                    <span className={`tag ${r.status === "approved" ? "tag-accent" : "tag-neutral"}`}>
                      {r.status === "approved" ? "granted for 4h" : "denied"}
                    </span>
                  )
                )}
              </div>
            );
          })}
        </div>
      </section>

      {assist.grants.length > 0 && (
        <section>
          <SectionTitle>Active grants</SectionTitle>
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            {assist.grants.map((g) => (
              <div
                key={g.scope_request_id}
                className="card"
                style={{ display: "flex", alignItems: "center", gap: 10, padding: "9px 14px", fontSize: 12.5 }}
              >
                <span className="tag tag-accent">{g.kind}</span>
                <span className="mono" style={{ flex: 1 }}>
                  {g.target ?? "-"}
                </span>
                <span style={{ color: "var(--color-neutral-600)" }}>{g.granted_to_name}</span>
                <span style={{ color: "var(--color-neutral-500)", fontSize: 11.5 }}>
                  {g.expires_at ? `expires ${timeAgo(g.expires_at)}` : "until close"}
                </span>
              </div>
            ))}
          </div>
        </section>
      )}

      <div>
        <button className="btn btn-primary" onClick={onClose}>
          <Lock size={13} />
          Close assist
        </button>
      </div>
    </div>
  );
}

function ResponderPanel({
  assist,
  liveData,
  isLive,
  grantFor,
  myPending,
  busy,
  act,
  onOpenFile,
  onRequestArtifacts,
}: {
  assist: AssistDetailT;
  liveData: LiveData | null;
  isLive: boolean;
  grantFor: (kind: ScopeKind) => Grant | undefined;
  myPending: (kind: ScopeKind) => ScopeRequest | undefined;
  busy: boolean;
  act: (fn: () => Promise<unknown>) => Promise<void>;
  onOpenFile: (path: string) => void;
  onRequestArtifacts: () => void;
}) {
  const [reason, setReason] = useState("");

  if (!assist.viewer_is_responder) {
    return (
      <div className="card" style={{ padding: 24, textAlign: "center" }}>
        <p style={{ margin: "0 0 12px", fontSize: 14 }}>
          Join to read granted scopes and help {assist.owner_name}.
        </p>
        <button
          className="btn btn-primary"
          disabled={busy}
          onClick={() => void act(() => apiPost(`/api/assists/${assist.ref}/responders`))}
        >
          Respond to this assist
        </button>
      </div>
    );
  }

  if (!isLive) {
    const pending = myPending("live_debug");
    return (
      <div className="card" style={{ padding: 24, textAlign: "center" }}>
        <h3 style={{ fontSize: 16, marginBottom: 6 }}>Live debug</h3>
        {pending ? (
          <span
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              color: "var(--color-neutral-600)",
            }}
          >
            <Spinner size={14} />
            Request sent. Waiting for {assist.owner_name}.
          </span>
        ) : (
          <>
            <p style={{ margin: "0 0 12px", fontSize: 13.5, color: "var(--color-neutral-600)" }}>
              Ask {assist.owner_name} to open a live, bounded view: granted files, a read-only
              terminal stream, and more on request.
            </p>
            <div style={{ display: "flex", gap: 8, justifyContent: "center" }}>
              <input
                className="input"
                style={{ maxWidth: 320 }}
                placeholder="Why live? e.g. quicker to trace this together"
                value={reason}
                onChange={(e) => setReason(e.target.value)}
              />
              <button
                className="btn btn-primary"
                disabled={busy}
                onClick={() =>
                  void act(() =>
                    apiPost(`/api/assists/${assist.ref}/scope-requests`, {
                      kind: "live_debug",
                      target: null,
                      reason: reason.trim() || "quicker to trace this together",
                      ttl_minutes: null,
                    }),
                  )
                }
              >
                <Radio size={13} />
                Request live debug
              </button>
            </div>
          </>
        )}
      </div>
    );
  }

  const agentsGrant = grantFor("agents");
  const agentsPending = myPending("agents");
  const sshGrant = grantFor("ssh");
  const sshPending = myPending("ssh");

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
        <button className="btn btn-dark" onClick={onRequestArtifacts}>
          Request artifacts
        </button>
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            fontSize: 12.5,
            fontWeight: 700,
            color: "var(--color-accent-700)",
          }}
        >
          <span
            className="pulse"
            style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--color-accent)" }}
          />
          Live
        </span>
        <span style={{ fontSize: 12, color: "var(--color-neutral-500)" }}>
          every grant expires automatically and is revocable by {assist.owner_name}
        </span>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(6, 1fr)", gap: 14 }}>
        <div className="card" style={{ gridColumn: "span 2", padding: 12 }}>
          <SectionTitle>Files and directories</SectionTitle>
          {liveData ? (
            <FileTree nodes={liveData.file_tree} onOpenFile={onOpenFile} />
          ) : (
            <Spinner size={14} />
          )}
        </div>

        <div style={{ gridColumn: "span 4" }}>
          {liveData && (
            <TerminalPane
              tabs={liveData.terminal_tabs.length > 0 ? liveData.terminal_tabs : ["terminal"]}
              feed={liveData.terminal_feed}
            />
          )}
        </div>

        <div className="card" style={{ gridColumn: "span 3", padding: 12 }}>
          <SectionTitle>Agents view</SectionTitle>
          {agentsGrant && liveData ? (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {liveData.agent_chat.length === 0 && (
                <span style={{ fontSize: 12.5, color: "var(--color-neutral-600)" }}>
                  No agent activity on this assist yet.
                </span>
              )}
              {liveData.agent_chat.map((m, i) => {
                const isAgent = m.who.startsWith("Agent");
                return (
                  <div
                    key={i}
                    style={{
                      background: isAgent ? "var(--color-neutral-100)" : "var(--color-accent-100)",
                      borderRadius: 8,
                      padding: "8px 10px",
                      fontSize: 12.5,
                    }}
                  >
                    <div style={{ fontWeight: 700, fontSize: 11, marginBottom: 2 }}>{m.who}</div>
                    {m.text}
                  </div>
                );
              })}
            </div>
          ) : (
            <GatedNote pending={!!agentsPending} label="Agents view" />
          )}
        </div>

        <div className="card" style={{ gridColumn: "span 3", padding: 12 }}>
          <SectionTitle>Device access / SSH</SectionTitle>
          {sshGrant ? (
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--color-success)" }}
              />
              <span className="mono" style={{ fontSize: 12.5 }}>
                {sshGrant.target ?? "staging host"}
              </span>
              <span className="tag tag-neutral">key forwarded</span>
            </div>
          ) : (
            <GatedNote pending={!!sshPending} label="Device access" />
          )}
        </div>
      </div>
    </div>
  );
}

function GatedNote({ pending, label }: { pending: boolean; label: string }) {
  return pending ? (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 7, fontSize: 12.5, color: "var(--color-neutral-600)" }}>
      <Spinner size={13} />
      {label}: awaiting owner approval
    </span>
  ) : (
    <span style={{ fontSize: 12.5, color: "var(--color-neutral-500)" }}>
      Not granted. Use "Request artifacts" and state why.
    </span>
  );
}

function RequestArtifactsModal({
  assist,
  onClose,
  onSend,
}: {
  assist: AssistDetailT;
  onClose: () => void;
  onSend: (kind: ScopeKind, target: string | null, reason: string) => void;
}) {
  const [step, setStep] = useState<"pick" | Exclude<ScopeKind, "comment" | "live_debug">>("pick");
  const [target, setTarget] = useState("");
  const [reason, setReason] = useState("");
  const [scanning, setScanning] = useState(false);

  const kinds: { kind: Exclude<ScopeKind, "comment" | "live_debug">; label: string; icon: typeof FileText }[] = [
    { kind: "file", label: "Files and directories", icon: FileText },
    { kind: "terminal", label: "Terminal stream", icon: SquareTerminal },
    { kind: "agents", label: "Agents view", icon: Bot },
    { kind: "ssh", label: "Device access (SSH)", icon: TerminalSquare },
  ];

  const grantedCount = (kind: ScopeKind) =>
    assist.grants.filter((g) => g.kind === kind).length;

  const pickStep = (kind: Exclude<ScopeKind, "comment" | "live_debug">) => {
    setStep(kind);
    setTarget("");
    setReason("");
    if (kind === "terminal" || kind === "agents") {
      setScanning(true);
      setTimeout(() => setScanning(false), 1200);
    }
  };

  const placeholders: Record<string, string> = {
    file: "path/to/file or directory/",
    terminal: "terminal name, e.g. iTerm2 (payments)",
    agents: "agent, e.g. Claude Code",
    ssh: "user@host",
  };

  return (
    <Modal width={460} onClose={onClose}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 14 }}>
        {step !== "pick" && (
          <button className="btn" style={{ padding: 6 }} onClick={() => setStep("pick")} title="Back">
            <ChevronLeft size={15} />
          </button>
        )}
        <h3 style={{ fontSize: 16 }}>
          {step === "pick" ? "Request artifacts" : kinds.find((k) => k.kind === step)?.label}
        </h3>
      </div>

      {step === "pick" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {kinds.map(({ kind, label, icon: Icon }) => (
            <button
              key={kind}
              className="btn"
              style={{ justifyContent: "flex-start" }}
              onClick={() => pickStep(kind)}
            >
              <Icon size={14} />
              {label}
              <span style={{ marginLeft: "auto", fontSize: 11, color: "var(--color-neutral-500)" }}>
                {grantedCount(kind) > 0 ? `${grantedCount(kind)} granted` : "not granted"}
              </span>
            </button>
          ))}
          <p style={{ fontSize: 12, color: "var(--color-neutral-600)", margin: "6px 0 0" }}>
            {assist.owner_name} approves each request. Your reason tells them what to look at.
          </p>
        </div>
      ) : scanning ? (
        <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
          <Spinner size={14} />
          {step === "terminal"
            ? "Listing active terminal sessions on the owner's machine..."
            : "Listing agent sessions on the owner's machine..."}
        </span>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="field">
            <label>Target</label>
            <input
              className="input mono"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              placeholder={placeholders[step]}
            />
          </div>
          <div className="field">
            <label>Reason (shown to {assist.owner_name})</label>
            <input
              className="input"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="why you need it"
            />
          </div>
          <button
            className="btn btn-primary"
            disabled={!reason.trim()}
            onClick={() => onSend(step, target.trim() || null, reason.trim())}
          >
            {step === "ssh" ? "Send SSH request" : "Request access"}
          </button>
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>
            Read-only, expires in 4h, revocable by the owner at any time.
          </p>
        </div>
      )}
    </Modal>
  );
}
