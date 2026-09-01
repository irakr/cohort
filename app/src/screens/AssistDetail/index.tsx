import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  Bot,
  Check,
  ChevronLeft,
  Clock,
  FileText,
  Folder,
  Lock,
  MessageCircle,
  Radio,
  Send,
  SquareTerminal,
  TerminalSquare,
  User as UserIcon,
  Users,
  X,
} from "lucide-react";
import {
  installSshKey,
  openSsh,
  snapshotPaths,
  sshPublicKey,
  sshTargetSuggestion,
  suggestArtifacts,
  terminalActivity,
} from "../../api/agent";
import { apiGet, apiPost } from "../../api/client";
import { useApi } from "../../api/hooks";
import type {
  AssistArtifact,
  AssistDetail as AssistDetailT,
  Grant,
  LiveData,
  ScopeKind,
  ScopeRequest,
} from "../../api/types";
import { FileTree } from "../../components/FileTree";
import { TerminalPane } from "../../components/TerminalPane";
import { AvatarChip, IconTile, Modal, SectionTitle, Spinner, StatusPill } from "../../components/ui";
import { CATEGORY_LABELS, renderMarkdown, timeAgo, timeUntil } from "../../util";
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
  // Polled so snapshot and terminal-activity updates from the owner's
  // engine reach responders without re-entering the screen.
  const { data: liveData } = useApi<LiveData>(
    isMember ? `/api/assists/${assistRef}/artifacts` : null,
    { pollMs: 5000 },
  );
  const [viewFile, setViewFile] = useState<string | null>(null);
  const [requestOpen, setRequestOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [sshApprove, setSshApprove] = useState<ScopeRequest | null>(null);

  // While the owner has an open assist on screen, their engine's current
  // scan (terminals, agents, suggested paths) is published to the hub so
  // the responder's request wizard can offer real options. Terminals with an
  // active grant additionally get their live process activity published as
  // the terminal feed (real PTY streaming arrives with the detector).
  const isOwnerOpen = !!assist && assist.viewer_is_owner && assist.status !== "done";
  const grantedTerminalsRef = useRef<string[]>([]);
  grantedTerminalsRef.current = assist
    ? assist.grants
        .filter((g) => g.kind === "terminal" && g.target !== null)
        .map((g) => g.target as string)
    : [];
  useEffect(() => {
    if (!isOwnerOpen) {
      return;
    }
    let cancelled = false;
    const publish = async () => {
      const groups = await suggestArtifacts();
      if (cancelled) {
        return;
      }
      const items = groups.flatMap((g) => g.items).map((it) => ({
        id: it.id,
        kind: it.kind,
        label: it.label,
        detail: it.detail,
        icon: it.icon,
        pid: it.pid,
      }));
      if (items.length === 0) {
        return; // no local engine (e.g. browser dev): publish nothing
      }
      try {
        await apiPost(`/api/assists/${assistRef}/catalog`, { items });
      } catch (e) {
        console.error("catalog publish failed:", e);
      }

      const granted = grantedTerminalsRef.current;
      if (granted.length === 0) {
        return;
      }
      const feed = await terminalActivity(granted);
      if (!feed || cancelled) {
        return;
      }
      try {
        // Merge onto the freshest live data so file snapshots survive.
        const current = await apiGet<LiveData>(`/api/assists/${assistRef}/artifacts`);
        await apiPost(`/api/assists/${assistRef}/artifacts`, {
          ...current,
          terminal_tabs: granted,
          terminal_feed: feed,
        });
      } catch (e) {
        console.error("terminal activity publish failed:", e);
      }
    };
    void publish();
    const timer = setInterval(() => void publish(), 10000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [isOwnerOpen, assistRef]);

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

  // Owner approval. SSH requests open a dedicated dialog (the owner supplies
  // the connection target and the responder's key gets installed). Approving
  // a file grant re-snapshots every shared and granted path locally and
  // uploads the full snapshot, so the responder's tree gains the new path
  // within a poll.
  async function approveRequest(r: ScopeRequest) {
    if (r.kind === "ssh") {
      setSshApprove(r);
      return;
    }
    await act(async () => {
      await apiPost(`/api/scope-requests/${r.id}/approve`);
      if (r.kind !== "file" || !assist) {
        return;
      }
      const paths = [
        ...assist.artifacts.filter((a) => a.kind === "file").map((a) => a.detail),
        ...assist.scope_requests
          .filter((s) => s.kind === "file" && s.status === "approved" && s.target)
          .map((s) => s.target as string),
        ...(r.target ? [r.target] : []),
      ];
      const snap = await snapshotPaths(paths);
      if (snap) {
        try {
          await apiPost(`/api/assists/${assist.ref}/artifacts`, {
            file_tree: snap.file_tree,
            files: snap.files,
            terminal_tabs: liveData?.terminal_tabs ?? [],
            terminal_feed: liveData?.terminal_feed ?? [],
            agent_chat: liveData?.agent_chat ?? [],
          });
        } catch (e) {
          console.error("live data upload failed:", e);
        }
      }
    });
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
        <OwnerPanel
          assist={assist}
          busy={busy}
          act={act}
          onApprove={approveRequest}
          onClose={() => navigate({ name: "close", ref: assist.ref })}
        />
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

      <Conversation assist={assist} busy={busy} act={act} />

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
          onSend={(kind, target, reason, payload) =>
            void act(async () => {
              await apiPost(`/api/assists/${assist.ref}/scope-requests`, {
                kind,
                target,
                reason,
                payload: payload ?? null,
                ttl_minutes: 240,
              });
              setRequestOpen(false);
            })
          }
        />
      )}

      {sshApprove && (
        <SshApproveModal
          assist={assist}
          request={sshApprove}
          busy={busy}
          onClose={() => setSshApprove(null)}
          onConfirm={(target) =>
            void act(async () => {
              // Install the responder's key first; without it the grant
              // would not actually work.
              if (sshApprove.payload) {
                const installed = await installSshKey(
                  sshApprove.payload,
                  `${assist.ref}:${sshApprove.id}`,
                );
                if (!installed) {
                  throw new Error(
                    "could not install the responder's SSH key on this machine",
                  );
                }
              }
              await apiPost(`/api/scope-requests/${sshApprove.id}/approve`, { target });
              setSshApprove(null);
            })
          }
        />
      )}
    </div>
  );
}

function SharedArtifactIcon({ artifact }: { artifact: AssistArtifact }) {
  if (artifact.icon) {
    return (
      <img
        src={artifact.icon}
        alt=""
        width={30}
        height={30}
        style={{ borderRadius: 8, flexShrink: 0, objectFit: "contain" }}
      />
    );
  }
  const Glyph =
    artifact.kind === "terminal"
      ? SquareTerminal
      : artifact.kind === "ai_agent"
        ? Bot
        : artifact.label.includes(".")
          ? FileText
          : Folder;
  return (
    <IconTile size={30} bg="var(--color-neutral-200)" fg="var(--color-neutral-700)">
      <Glyph size={15} />
    </IconTile>
  );
}

function Brief({ assist }: { assist: AssistDetailT }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20, marginBottom: 26 }}>
      {assist.description.trim() !== "" && (
        <div
          style={{ fontSize: 14, lineHeight: 1.6 }}
          dangerouslySetInnerHTML={{ __html: renderMarkdown(assist.description) }}
        />
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

      <section>
        <SectionTitle>Insights</SectionTitle>
        {assist.insights.trim() !== "" ? (
          <div
            style={{ fontSize: 14, lineHeight: 1.6 }}
            dangerouslySetInnerHTML={{ __html: renderMarkdown(assist.insights) }}
          />
        ) : (
          <p
            style={{ fontSize: 13, color: "var(--color-neutral-500)", margin: 0 }}
            title="The Cohort AI integration will analyze the shared artifacts and summarize the intent here"
          >
            N/A
          </p>
        )}
      </section>

      {assist.artifacts.length > 0 && (
        <section>
          <SectionTitle>Shared Artifacts</SectionTitle>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {assist.artifacts.map((a) => (
              <div
                key={a.id}
                className="card"
                style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 14px" }}
              >
                <SharedArtifactIcon artifact={a} />
                <span style={{ minWidth: 0, flex: 1 }}>
                  <span style={{ display: "block", fontSize: 13.5, fontWeight: 600 }}>
                    {a.label}
                  </span>
                  <span
                    className="mono"
                    style={{
                      display: "block",
                      fontSize: 11.5,
                      color: "var(--color-neutral-600)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {a.detail}
                  </span>
                </span>
                {a.pid !== null && (
                  <span className="tag tag-neutral mono" style={{ fontSize: 11 }}>
                    pid {a.pid}
                  </span>
                )}
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

// Comments live in scope_requests (kind "comment", auto-approved); this is
// the conversation between the owner and responders.
function Conversation({
  assist,
  busy,
  act,
}: {
  assist: AssistDetailT;
  busy: boolean;
  act: (fn: () => Promise<unknown>) => Promise<void>;
}) {
  const [draft, setDraft] = useState("");
  const comments = assist.scope_requests.filter((r) => r.kind === "comment");
  const canPost =
    (assist.viewer_is_owner || assist.viewer_is_responder) && assist.status !== "done";

  async function send() {
    const text = draft.trim();
    if (text === "") {
      return;
    }
    await act(async () => {
      await apiPost(`/api/assists/${assist.ref}/scope-requests`, {
        kind: "comment",
        target: null,
        reason: text,
        ttl_minutes: null,
      });
      setDraft("");
    });
  }

  return (
    <section style={{ marginTop: 24 }}>
      <SectionTitle>Conversation</SectionTitle>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {comments.length === 0 && (
          <p style={{ fontSize: 13, color: "var(--color-neutral-500)", margin: 0 }}>
            No comments yet.
          </p>
        )}
        {comments.map((c) => (
          <div key={c.id} style={{ display: "flex", gap: 10, alignItems: "flex-start" }}>
            <AvatarChip name={c.requester_name} size={26} active={c.requester_id === assist.owner_id} />
            <div className="card" style={{ padding: "8px 12px", flex: 1 }}>
              <div style={{ display: "flex", gap: 8, alignItems: "baseline", marginBottom: 2 }}>
                <span style={{ fontSize: 12.5, fontWeight: 700 }}>{c.requester_name}</span>
                {c.requester_id === assist.owner_id && (
                  <span className="tag tag-accent" style={{ fontSize: 10 }}>
                    owner
                  </span>
                )}
                <span style={{ fontSize: 11, color: "var(--color-neutral-500)" }}>
                  {timeAgo(c.created_at)} ago
                </span>
              </div>
              <div style={{ fontSize: 13.5 }}>{c.reason}</div>
            </div>
          </div>
        ))}
        {canPost && (
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input"
              placeholder={
                assist.viewer_is_owner
                  ? "Reply to your responders"
                  : `Comment for ${assist.owner_name}`
              }
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  void send();
                }
              }}
            />
            <button
              className="btn btn-primary"
              title="Send comment"
              disabled={busy || draft.trim() === ""}
              onClick={() => void send()}
            >
              <Send size={14} />
            </button>
          </div>
        )}
      </div>
    </section>
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
  onApprove,
  onClose,
}: {
  assist: AssistDetailT;
  busy: boolean;
  act: (fn: () => Promise<unknown>) => Promise<void>;
  onApprove: (r: ScopeRequest) => Promise<void>;
  onClose: () => void;
}) {
  // Comments live in the Conversation section; this list is actionable only.
  const requests = assist.scope_requests.filter((r) => r.kind !== "comment");
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <section>
        <SectionTitle>Responds</SectionTitle>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {requests.length === 0 && (
            <div className="card" style={{ padding: 16, fontSize: 13, color: "var(--color-neutral-600)" }}>
              Nothing yet. Scope requests from responders land here for one-tap approval.
            </div>
          )}
          {requests.map((r) => {
            const Icon = KIND_ICON[r.kind];
            return (
              <div
                key={r.id}
                className="card"
                style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 14px" }}
              >
                <IconTile size={26} bg="var(--color-accent-100)" fg="var(--color-accent-700)">
                  <Icon size={13} />
                </IconTile>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13.5, fontWeight: 600 }}>{requestTitle(r)}</div>
                  <div style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>
                    {`${r.requester_name}: "${r.reason}"`}
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
                      onClick={() => void onApprove(r)}
                    >
                      <Check size={14} />
                    </button>
                  </>
                ) : (
                  <span className={`tag ${r.status === "approved" ? "tag-accent" : "tag-neutral"}`}>
                    {r.status === "approved" ? "granted for 4h" : "denied"}
                  </span>
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
                  {g.expires_at ? `expires ${timeUntil(g.expires_at)}` : "until close"}
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
          {liveData && liveData.terminal_feed.length > 0 ? (
            <TerminalPane
              tabs={liveData.terminal_tabs.length > 0 ? liveData.terminal_tabs : ["terminal"]}
              feed={liveData.terminal_feed}
            />
          ) : (
            <div className="card" style={{ padding: 16 }}>
              <SectionTitle>Terminal view</SectionTitle>
              <p style={{ fontSize: 12.5, color: "var(--color-neutral-500)", margin: 0 }}>
                {grantFor("terminal") ? (
                  <>
                    <Spinner size={12} /> Terminal granted - waiting for{" "}
                    {assist.owner_name}'s engine to publish activity (it
                    updates while they view this assist).
                  </>
                ) : (
                  <>
                    No terminal granted yet - use "Request artifacts" to ask
                    for one. The view shows live process activity; full
                    output streaming arrives with the detector.
                  </>
                )}
              </p>
            </div>
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
          {sshGrant && sshGrant.target ? (
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--color-success)" }}
              />
              <span className="mono" style={{ fontSize: 12.5, flex: 1 }}>
                {sshGrant.target}
              </span>
              <button
                className="btn btn-primary"
                style={{ padding: "5px 12px", fontSize: 12 }}
                title="Open an SSH session in your terminal"
                onClick={() => void openSsh(sshGrant.target as string)}
              >
                Connect
              </button>
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

function CatalogPickList({
  items,
  emptyText,
  selected,
  connected = [],
  onSelect,
}: {
  items: AssistArtifact[];
  emptyText: string;
  selected: string;
  /** Labels already granted to this responder: shown Connected, not selectable. */
  connected?: string[];
  onSelect: (label: string) => void;
}) {
  if (items.length === 0) {
    return <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: 0 }}>{emptyText}</p>;
  }
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {items.map((item) => {
        const isConnected = connected.includes(item.label);
        const on = !isConnected && selected === item.label;
        return (
          <button
            key={item.id}
            className="btn"
            disabled={isConnected}
            title={isConnected ? "Already granted to you" : undefined}
            style={{
              justifyContent: "flex-start",
              gap: 10,
              boxShadow: on ? "inset 0 0 0 2px var(--color-accent)" : undefined,
              background: on ? "var(--color-accent-100)" : undefined,
            }}
            onClick={() => onSelect(item.label)}
          >
            <SharedArtifactIcon artifact={item} />
            <span style={{ minWidth: 0, textAlign: "left", flex: 1 }}>
              <span style={{ display: "block", fontSize: 13 }}>{item.label}</span>
              <span
                style={{
                  display: "block",
                  fontSize: 11,
                  fontWeight: 400,
                  color: "var(--color-neutral-600)",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {item.detail}
              </span>
            </span>
            {isConnected ? (
              <span className="tag tag-accent" style={{ fontSize: 10.5 }}>
                Connected
              </span>
            ) : (
              item.pid !== null && (
                <span className="tag tag-neutral mono" style={{ fontSize: 10.5 }}>
                  pid {item.pid}
                </span>
              )
            )}
          </button>
        );
      })}
    </div>
  );
}

// The request wizard is populated by the owner's engine: the owner's app
// publishes its current scan (catalog) to the hub while the assist is open,
// and the responder picks from real running terminals/agents and suggested
// paths. SSH carries this machine's public key with the request.
function RequestArtifactsModal({
  assist,
  onClose,
  onSend,
}: {
  assist: AssistDetailT;
  onClose: () => void;
  onSend: (kind: ScopeKind, target: string | null, reason: string, payload?: string) => void;
}) {
  const me = getCurrentUserId();
  const [step, setStep] = useState<"pick" | Exclude<ScopeKind, "comment" | "live_debug">>("pick");
  const [target, setTarget] = useState("");
  const [reason, setReason] = useState("");
  // undefined = still reading; null = no key on this machine.
  const [myKey, setMyKey] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    if (step === "ssh" && myKey === undefined) {
      void sshPublicKey().then(setMyKey);
    }
  }, [step, myKey]);

  const kinds: { kind: Exclude<ScopeKind, "comment" | "live_debug">; label: string; icon: typeof FileText }[] = [
    { kind: "file", label: "Files and directories", icon: FileText },
    { kind: "terminal", label: "Terminal stream", icon: SquareTerminal },
    { kind: "agents", label: "Agents view", icon: Bot },
    { kind: "ssh", label: "Device access (SSH)", icon: TerminalSquare },
  ];

  const grantedCount = (kind: ScopeKind) => assist.grants.filter((g) => g.kind === kind).length;
  const catalogFor = (kind: "terminal" | "ai_agent" | "file") =>
    assist.catalog.filter((i) => i.kind === kind);
  const scannedNote =
    assist.catalog_at !== null
      ? `Scanned on ${assist.owner_name}'s machine ${timeAgo(assist.catalog_at)} ago.`
      : `${assist.owner_name}'s engine has not scanned yet - it publishes while they view this assist.`;

  const pickStep = (kind: Exclude<ScopeKind, "comment" | "live_debug">) => {
    setStep(kind);
    setTarget("");
    setReason("");
  };

  const reasonField = (
    <div className="field">
      <label>Reason (shown to {assist.owner_name})</label>
      <input
        className="input"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder="why you need it"
      />
    </div>
  );

  // Previously provided SSH access for this responder, newest last.
  const myCredentials = assist.scope_requests.filter(
    (r) => r.kind === "ssh" && r.requester_id === me && r.status === "approved" && r.target,
  );
  const sshPendingMine = assist.scope_requests.some(
    (r) => r.kind === "ssh" && r.requester_id === me && r.status === "pending",
  );
  const grantActive = (requestId: number) =>
    assist.grants.some((g) => g.scope_request_id === requestId);

  return (
    <Modal width={480} onClose={onClose}>
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

      {step === "pick" && (
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
      )}

      {(step === "terminal" || step === "agents") && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <CatalogPickList
            items={catalogFor(step === "terminal" ? "terminal" : "ai_agent")}
            emptyText={
              step === "terminal"
                ? `No terminals are running on ${assist.owner_name}'s machine right now.`
                : `No AI agents are running on ${assist.owner_name}'s machine right now.`
            }
            selected={target}
            connected={assist.grants
              .filter((g) => g.kind === step && g.granted_to_id === me && g.target !== null)
              .map((g) => g.target as string)}
            onSelect={setTarget}
          />
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>{scannedNote}</p>
          {reasonField}
          <button
            className="btn btn-primary"
            disabled={!reason.trim() || !target}
            onClick={() => onSend(step, target, reason.trim())}
          >
            Request access
          </button>
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>
            Read-only, expires in 4h, revocable by the owner at any time.
          </p>
        </div>
      )}

      {step === "file" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {catalogFor("file").length > 0 && (
            <div className="field">
              <label>Suggested by {assist.owner_name}'s engine</label>
              <CatalogPickList
                items={catalogFor("file")}
                emptyText=""
                selected={target}
                onSelect={(label) => {
                  const item = catalogFor("file").find((i) => i.label === label);
                  setTarget(item?.detail ?? label);
                }}
              />
            </div>
          )}
          <div className="field">
            <label>Path</label>
            <input
              className="input mono"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              placeholder="path/to/file or directory/"
            />
          </div>
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>{scannedNote}</p>
          {reasonField}
          <button
            className="btn btn-primary"
            disabled={!reason.trim() || !target.trim()}
            onClick={() => onSend("file", target.trim(), reason.trim())}
          >
            Request access
          </button>
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>
            Read-only, expires in 4h, revocable by the owner at any time.
          </p>
        </div>
      )}

      {step === "ssh" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {myCredentials.length > 0 && (
            <div className="field">
              <label>Provided access</label>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {myCredentials.map((c) => (
                  <div
                    key={c.id}
                    className="card"
                    style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 12px" }}
                  >
                    <span className="mono" style={{ fontSize: 12.5, flex: 1 }}>
                      {c.target}
                    </span>
                    {grantActive(c.id) ? (
                      <button
                        className="btn btn-primary"
                        style={{ padding: "5px 12px", fontSize: 12 }}
                        title="Open an SSH session in your terminal"
                        onClick={() => void openSsh(c.target as string)}
                      >
                        Connect
                      </button>
                    ) : (
                      <span className="tag tag-neutral">expired</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {sshPendingMine ? (
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
              <Spinner size={14} />
              Request sent. Waiting for {assist.owner_name}.
            </span>
          ) : myKey === undefined ? (
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
              <Spinner size={14} /> Reading this machine's SSH key...
            </span>
          ) : myKey === null ? (
            <p style={{ fontSize: 12.5, color: "var(--color-warning-fg)", margin: 0 }}>
              No SSH key found on this machine. Create one with ssh-keygen, then
              reopen this dialog.
            </p>
          ) : (
            <>
              <div className="field">
                <label>Your public key (sent with the request)</label>
                <div
                  className="mono"
                  style={{
                    fontSize: 11,
                    background: "var(--color-neutral-100)",
                    borderRadius: 8,
                    padding: "6px 10px",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {myKey}
                </div>
              </div>
              {reasonField}
              <button
                className="btn btn-primary"
                disabled={!reason.trim()}
                onClick={() => onSend("ssh", null, reason.trim(), myKey)}
              >
                Request SSH access
              </button>
              <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>
                On approval, {assist.owner_name} authorizes this key on their
                machine and shares the connection target - passwordless, no
                secrets stored.
              </p>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}

// Owner-side SSH approval: shows the responder's key, takes the connection
// target, and (on confirm) installs the key into authorized_keys before the
// hub approval is recorded.
function SshApproveModal({
  assist,
  request,
  busy,
  onClose,
  onConfirm,
}: {
  assist: AssistDetailT;
  request: ScopeRequest;
  busy: boolean;
  onClose: () => void;
  onConfirm: (target: string) => void;
}) {
  const [target, setTarget] = useState("");

  useEffect(() => {
    void sshTargetSuggestion().then((s) => setTarget((prev) => (prev === "" ? s : prev)));
  }, []);

  return (
    <Modal width={480} onClose={onClose}>
      <h3 style={{ fontSize: 16, marginBottom: 4 }}>Grant SSH access</h3>
      <p style={{ fontSize: 13, color: "var(--color-neutral-600)", margin: "0 0 12px" }}>
        {request.requester_name}: "{request.reason}"
      </p>
      {request.payload ? (
        <div className="field" style={{ marginBottom: 10 }}>
          <label>{request.requester_name}'s public key</label>
          <div
            className="mono"
            style={{
              fontSize: 11,
              background: "var(--color-neutral-100)",
              borderRadius: 8,
              padding: "6px 10px",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {request.payload}
          </div>
        </div>
      ) : (
        <p style={{ fontSize: 12.5, color: "var(--color-warning-fg)", margin: "0 0 10px" }}>
          No public key came with this request; the responder will need their
          own way to authenticate.
        </p>
      )}
      <div className="field" style={{ marginBottom: 12 }}>
        <label>Connection target they will use</label>
        <input
          className="input mono"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder="user@host"
        />
      </div>
      <button
        className="btn btn-primary btn-block"
        disabled={busy || target.trim() === ""}
        onClick={() => onConfirm(target.trim())}
      >
        Authorize key and grant
      </button>
      <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: "10px 0 0" }}>
        The key is added to ~/.ssh/authorized_keys on this machine, tagged
        cohort:{assist.ref}:{request.id}. Until revoke ships, withdraw access
        by deleting that tagged line.
      </p>
    </Modal>
  );
}
