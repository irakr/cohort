import { useEffect, useRef, useState } from "react";
import {
  AppWindow,
  ArrowLeft,
  Bot,
  Check,
  Clock,
  FileText,
  Folder,
  Lock,
  MessageCircle,
  Plus,
  Radio,
  Send,
  Trash2,
  TerminalSquare,
  User as UserIcon,
  Users,
  X,
} from "lucide-react";
import {
  captureWindow,
  installSshKey,
  openSsh,
  snapshotPaths,
  sshPublicKey,
  sshTargetSuggestion,
  suggestArtifacts,
} from "../../api/agent";
import { apiDelete, apiGetBlob, apiPost, apiPutBytes } from "../../api/client";
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
import {
  AvatarChip,
  ConfirmDialog,
  IconTile,
  Modal,
  NoticeDialog,
  SectionTitle,
  Spinner,
  StatusPill,
} from "../../components/ui";
import { CATEGORY_LABELS, renderMarkdown, timeAgo, timeUntil } from "../../util";
import { useNav } from "../../app/router";
import { getCurrentUserId } from "../../api/currentUser";

/** Friendly form of a grant target; window targets carry "w-<id>|" first. */
function displayTarget(kind: ScopeKind, target: string | null): string {
  if (target === null) {
    return "-";
  }
  if (kind === "window" && target.includes("|")) {
    return target.split("|").slice(1).join("|");
  }
  return target;
}

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
  // Polled so snapshot updates from the owner's engine reach responders
  // without re-entering the screen.
  const { data: liveData } = useApi<LiveData>(
    isMember ? `/api/assists/${assistRef}/artifacts` : null,
    { pollMs: 5000 },
  );
  const [viewFile, setViewFile] = useState<string | null>(null);
  const [requestOpen, setRequestOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [sshApprove, setSshApprove] = useState<ScopeRequest | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [windowApprove, setWindowApprove] = useState<ScopeRequest | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // While the owner has an open assist on screen, their engine's current
  // scan (agents, windows, suggested paths) is published to the hub so
  // the responder's request wizard can offer real options.
  const isOwnerOpen = !!assist && assist.viewer_is_owner && assist.status !== "done";
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
    };
    void publish();
    const timer = setInterval(() => void publish(), 10000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [isOwnerOpen, assistRef]);

  // Owner side: stream the granted windows as JPEG frames (~1.5s cadence)
  // while this assist is on screen. The hub relays only the newest frame.
  const grantedWindowsRef = useRef<{ id: number; target: string }[]>([]);
  grantedWindowsRef.current = assist
    ? assist.grants
        .filter((g) => g.kind === "window" && g.target !== null)
        .map((g) => ({ id: g.scope_request_id, target: g.target as string }))
    : [];
  useEffect(() => {
    if (!isOwnerOpen) {
      return;
    }
    let cancelled = false;
    const tick = async () => {
      for (const w of grantedWindowsRef.current) {
        const bytes = await captureWindow(w.target);
        if (cancelled) {
          return;
        }
        if (!bytes) {
          continue;
        }
        try {
          await apiPutBytes(`/api/assists/${assistRef}/frames/${w.id}`, bytes);
        } catch (e) {
          console.error("frame upload failed:", e);
        }
      }
    };
    void tick();
    const timer = setInterval(() => void tick(), 1500);
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
      setNotice(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  // Owner-only, works for open and closed assists alike; deleting a closed
  // assist also removes its resolution record and granted credits. The
  // confirmation is the in-app ConfirmDialog rendered below.
  async function performDelete() {
    if (!assist) {
      return;
    }
    setConfirmDelete(false);
    setBusy(true);
    try {
      await apiDelete(`/api/assists/${assist.ref}`);
      navigate({ name: "assists" });
    } catch (e) {
      setNotice(e instanceof Error ? e.message : String(e));
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
    if (r.kind === "window") {
      setWindowApprove(r);
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

      {assist.viewer_is_owner &&
        assist.grants.some((g) => g.kind === "window") && (
          <div
            style={{
              position: "sticky",
              top: 10,
              zIndex: 40,
              background: "var(--color-accent-100)",
              border: "1px solid var(--color-accent-300)",
              borderRadius: 10,
              padding: "10px 14px",
              marginBottom: 18,
              display: "flex",
              flexDirection: "column",
              gap: 6,
            }}
          >
            {assist.grants
              .filter((g) => g.kind === "window")
              .map((g) => (
                <div key={g.scope_request_id} style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <span
                    className="pulse"
                    style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--color-accent)" }}
                  />
                  <span style={{ fontSize: 12.5, fontWeight: 700, color: "var(--color-accent-800)", flex: 1 }}>
                    Sharing window: {displayTarget("window", g.target)} with {g.granted_to_name}
                  </span>
                  <button
                    className="btn"
                    style={{ padding: "4px 12px", fontSize: 12, color: "var(--color-accent-700)" }}
                    disabled={busy}
                    onClick={() =>
                      void act(() => apiPost(`/api/scope-requests/${g.scope_request_id}/revoke`))
                    }
                  >
                    Stop
                  </button>
                </div>
              ))}
          </div>
        )}

      <Brief assist={assist} />

      {assist.viewer_is_owner && assist.status !== "done" && (
        <OwnerPanel
          assist={assist}
          busy={busy}
          act={act}
          onApprove={approveRequest}
          onClose={() => navigate({ name: "close", ref: assist.ref })}
          onDelete={() => setConfirmDelete(true)}
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
          onNotice={setNotice}
        />
      )}

      {assist.status === "done" && (
        <div
          className="card"
          style={{
            padding: 18,
            fontSize: 13.5,
            color: "var(--color-neutral-600)",
            display: "flex",
            alignItems: "center",
            gap: 12,
          }}
        >
          <span style={{ flex: 1 }}>
            This assist is closed. Its resolution record is kept until the
            assist is deleted.
          </span>
          {assist.viewer_is_owner && (
            <button
              className="btn"
              style={{ color: "var(--color-accent-700)", flexShrink: 0 }}
              title="Delete this assist, its resolution record and granted credits"
              disabled={busy}
              onClick={() => setConfirmDelete(true)}
            >
              <Trash2 size={13} />
              Delete assist
            </button>
          )}
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
          busy={busy}
          onNotice={setNotice}
          onClose={() => setRequestOpen(false)}
          onSubmit={(requests, reason) =>
            void act(async () => {
              for (const request of requests) {
                await apiPost(`/api/assists/${assist.ref}/scope-requests`, {
                  kind: request.kind,
                  target: request.target,
                  reason,
                  payload: request.payload,
                  ttl_minutes: 240,
                });
              }
              setRequestOpen(false);
            })
          }
        />
      )}

      {confirmDelete && (
        <ConfirmDialog
          title="Delete assist"
          message={
            assist.status === "done"
              ? `Delete ${assist.ref}? Its resolution record and any credits it granted are removed too. This cannot be undone.`
              : `Delete ${assist.ref} and everything shared on it? This cannot be undone.`
          }
          confirmLabel="Delete"
          busy={busy}
          onCancel={() => setConfirmDelete(false)}
          onConfirm={() => void performDelete()}
        />
      )}

      {notice && <NoticeDialog message={notice} onClose={() => setNotice(null)} />}

      {windowApprove && (
        <ConfirmDialog
          title="Share this window?"
          message={`${windowApprove.requester_name} will see "${displayTarget("window", windowApprove.target)}" live, pixel for pixel. Everything visible in that window is shared raw - nothing can be redacted. View-only; a sharing indicator with a Stop button stays on this assist, and the grant expires automatically.`}
          confirmLabel="Share window"
          busy={busy}
          onCancel={() => setWindowApprove(null)}
          onConfirm={() => {
            const request = windowApprove;
            setWindowApprove(null);
            void act(() => apiPost(`/api/scope-requests/${request.id}/approve`));
          }}
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
    artifact.kind === "ai_agent"
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
  agents: Bot,
  ssh: TerminalSquare,
  window: AppWindow,
};

function requestTitle(r: ScopeRequest): string {
  if (r.kind === "comment") {
    return r.reason;
  }
  if (r.kind === "live_debug") {
    return "Live debug request";
  }
  return displayTarget(r.kind, r.target) === "-" ? r.kind : displayTarget(r.kind, r.target);
}

function OwnerPanel({
  assist,
  busy,
  act,
  onApprove,
  onClose,
  onDelete,
}: {
  assist: AssistDetailT;
  busy: boolean;
  act: (fn: () => Promise<unknown>) => Promise<void>;
  onApprove: (r: ScopeRequest) => Promise<void>;
  onClose: () => void;
  onDelete: () => void;
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
                  {displayTarget(g.kind, g.target)}
                </span>
                <span style={{ color: "var(--color-neutral-600)" }}>{g.granted_to_name}</span>
                <span style={{ color: "var(--color-neutral-500)", fontSize: 11.5 }}>
                  {g.expires_at ? `expires ${timeUntil(g.expires_at)}` : "until close"}
                </span>
                <button
                  className="btn"
                  style={{ padding: "3px 10px", fontSize: 11.5, color: "var(--color-accent-700)" }}
                  title="Revoke this grant now"
                  disabled={busy}
                  onClick={() =>
                    void act(() => apiPost(`/api/scope-requests/${g.scope_request_id}/revoke`))
                  }
                >
                  Revoke
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      <div style={{ display: "flex", gap: 10 }}>
        <button className="btn btn-primary" onClick={onClose}>
          <Lock size={13} />
          Close assist
        </button>
        <button
          className="btn"
          style={{ color: "var(--color-accent-700)" }}
          title="Delete this assist and everything shared on it"
          disabled={busy}
          onClick={onDelete}
        >
          <Trash2 size={13} />
          Delete assist
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
  onNotice,
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
  onNotice: (message: string) => void;
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
              Ask {assist.owner_name} to open a live, bounded view: granted files,
              application windows, and more on request.
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
            // Height-capped and scrollable so deep trees never stretch
            // the grid.
            <div style={{ maxHeight: 300, overflow: "auto" }}>
              <FileTree nodes={liveData.file_tree} onOpenFile={onOpenFile} />
            </div>
          ) : (
            <Spinner size={14} />
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
                onClick={() =>
                  void openSsh(sshGrant.target as string).then((err) => {
                    if (err) onNotice(`Could not open an SSH terminal. ${err}`);
                  })
                }
              >
                Connect
              </button>
            </div>
          ) : (
            <GatedNote pending={!!sshPending} label="Device access" />
          )}
        </div>

        {grantFor("window") && (
          <div style={{ gridColumn: "1 / -1" }}>
            <WindowPane assistRef={assist.ref} grant={grantFor("window") as Grant} />
          </div>
        )}
      </div>
    </div>
  );
}

// Live view of a granted application window: the owner's engine streams
// JPEG frames through the hub's in-memory relay; this pane follows them.
function WindowPane({ assistRef, grant }: { assistRef: string; grant: Grant }) {
  const [src, setSrc] = useState<string | null>(null);
  const [stale, setStale] = useState(true);

  useEffect(() => {
    let stopped = false;
    let currentUrl: string | null = null;
    const tick = async () => {
      try {
        const blob = await apiGetBlob(`/api/assists/${assistRef}/frames/${grant.scope_request_id}`);
        if (stopped) {
          return;
        }
        if (blob) {
          const url = URL.createObjectURL(blob);
          setSrc((prev) => {
            if (prev) {
              URL.revokeObjectURL(prev);
            }
            return url;
          });
          currentUrl = url;
          setStale(false);
        } else {
          setStale(true);
        }
      } catch {
        if (!stopped) {
          setStale(true);
        }
      }
    };
    void tick();
    const timer = setInterval(() => void tick(), 1500);
    return () => {
      stopped = true;
      clearInterval(timer);
      if (currentUrl) {
        URL.revokeObjectURL(currentUrl);
      }
    };
  }, [assistRef, grant.scope_request_id]);

  return (
    <div className="card" style={{ padding: 12 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
        <AppWindow size={14} color="var(--color-neutral-700)" />
        <span style={{ fontSize: 13, fontWeight: 700 }}>
          {displayTarget("window", grant.target)}
        </span>
        <span
          className={stale ? "tag tag-neutral" : "tag tag-accent"}
          style={{ marginLeft: "auto", fontSize: 10.5 }}
        >
          {stale ? "waiting for the owner" : "live - view only"}
        </span>
      </div>
      {src ? (
        <img
          src={src}
          alt="Shared application window"
          style={{ width: "100%", borderRadius: 8, display: "block" }}
        />
      ) : (
        <p style={{ fontSize: 12.5, color: "var(--color-neutral-500)", margin: 0 }}>
          No frame yet. The owner's engine streams while they view this assist.
        </p>
      )}
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

// The redesigned request wizard: category tabs on the left, compact
// multi-select rows on the right, one batch "Request access". Options come
// from the owner-published catalog; already-granted items show as shared.
interface ReqRow {
  key: string;
  kind: ScopeKind;
  label: string;
  detail: string;
  icon: string | null;
  granted: boolean;
  selectable: boolean;
  target: string | null;
}

function ReqIcon({ row }: { row: ReqRow }) {
  if (row.icon) {
    return (
      <img
        src={row.icon}
        alt=""
        width={19}
        height={19}
        style={{ borderRadius: 5, flexShrink: 0, objectFit: "contain" }}
      />
    );
  }
  const Glyph =
    row.kind === "agents"
      ? Bot
      : row.kind === "window"
        ? AppWindow
        : row.kind === "ssh"
          ? TerminalSquare
          : row.label.includes(".")
            ? FileText
            : Folder;
  const color =
    row.kind === "file" && !row.label.includes(".")
      ? "#64a8e8"
      : row.label.includes("Claude")
        ? "#d97757"
        : "var(--color-neutral-600)";
  return <Glyph size={15} color={color} style={{ flexShrink: 0 }} />;
}

interface ReqCategory {
  id: string;
  name: string;
  icon: typeof FileText;
  desc: string;
  subs: { name: string; rows: ReqRow[] }[];
  custom?: boolean;
  note?: string;
}

function RequestArtifactsModal({
  assist,
  busy,
  onNotice,
  onClose,
  onSubmit,
}: {
  assist: AssistDetailT;
  busy: boolean;
  onNotice: (message: string) => void;
  onClose: () => void;
  onSubmit: (
    requests: { kind: ScopeKind; target: string | null; payload: string | null }[],
    reason: string,
  ) => void;
}) {
  const me = getCurrentUserId();
  const [activeId, setActiveId] = useState("files");
  const [picked, setPicked] = useState<Record<string, boolean>>({});
  const [chips, setChips] = useState<string[]>([]);
  const [fileDraft, setFileDraft] = useState("");
  const [reason, setReason] = useState("");
  // undefined = still reading; null = no key on this machine.
  const [myKey, setMyKey] = useState<string | null | undefined>(undefined);

  useEffect(() => {
    void sshPublicKey().then(setMyKey);
  }, []);

  const myGrants = assist.grants.filter((g) => g.granted_to_id === me);
  const grantedTargets = (kind: ScopeKind) =>
    myGrants.filter((g) => g.kind === kind && g.target !== null).map((g) => g.target as string);
  const catalogFor = (kind: string) => assist.catalog.filter((i) => i.kind === kind);
  const sshPendingMine = assist.scope_requests.some(
    (r) => r.kind === "ssh" && r.requester_id === me && r.status === "pending",
  );
  const myCredentials = assist.scope_requests.filter(
    (r) => r.kind === "ssh" && r.requester_id === me && r.status === "approved" && r.target,
  );
  const grantActive = (requestId: number) =>
    assist.grants.some((g) => g.scope_request_id === requestId);

  const catalogRow = (item: AssistArtifact, kind: ScopeKind, target: string): ReqRow => {
    const granted = grantedTargets(kind).includes(target);
    return {
      key: `${kind}:${target}`,
      kind,
      label: item.label,
      detail: item.detail,
      icon: item.icon,
      granted,
      selectable: !granted,
      target,
    };
  };

  const categories: ReqCategory[] = [
    {
      id: "files",
      name: "Files & Directories",
      icon: FileText,
      desc: "Ask for read access to paths. Snapshotted read-only and redacted.",
      custom: true,
      subs: [
        {
          name: "Already shared",
          rows: grantedTargets("file").map((t) => ({
            key: `shared:${t}`,
            kind: "file" as ScopeKind,
            label: t.split("/").filter(Boolean).pop() ?? t,
            detail: t,
            icon: null,
            granted: true,
            selectable: false,
            target: t,
          })),
        },
        {
          name: "Suggested",
          rows: catalogFor("file").map((i) => catalogRow(i, "file", i.detail)),
        },
        {
          name: "Added by you",
          rows: chips.map((p) => ({
            key: `file:${p}`,
            kind: "file" as ScopeKind,
            label: p.split("/").filter(Boolean).pop() ?? p,
            detail: p,
            icon: null,
            granted: false,
            selectable: true,
            target: p,
          })),
        },
      ],
    },
    {
      id: "agents",
      name: "AI agents",
      icon: Bot,
      desc: "See the agent run's errors and context. Read-only.",
      subs: [
        {
          name: "Detected",
          rows: catalogFor("ai_agent").map((i) => catalogRow(i, "agents", i.label)),
        },
      ],
      note: `No AI agents are running on ${assist.owner_name}'s machine right now.`,
    },
    {
      id: "appwin",
      name: "Application window",
      icon: AppWindow,
      desc: "Streams the window pixel-for-pixel, like a screenshare. View-only - you never control it.",
      subs: [
        {
          name: "Open windows",
          rows: catalogFor("window").map((i) =>
            catalogRow(i, "window", `${i.id}|${i.label}: ${i.detail}`),
          ),
        },
      ],
      note: `No application windows detected on ${assist.owner_name}'s machine right now.`,
    },
    {
      id: "ssh",
      name: "Device access",
      icon: TerminalSquare,
      desc: "Encrypted end-to-end. The owner authorizes your key and shares the address after approval.",
      subs: [
        {
          name: "Connection",
          rows: [
            {
              key: "ssh",
              kind: "ssh" as ScopeKind,
              label: "SSH",
              detail: sshPendingMine
                ? `request pending - waiting for ${assist.owner_name}`
                : myKey === null
                  ? "no SSH key on this machine - create one with ssh-keygen"
                  : myKey === undefined
                    ? "reading this machine's SSH key..."
                    : "your public key is sent; the owner shares the address",
              icon: null,
              granted: false,
              selectable: !sshPendingMine && typeof myKey === "string",
              target: null,
            },
          ],
        },
      ],
    },
  ];

  const active = categories.find((c) => c.id === activeId) ?? categories[0];
  const allRows = categories.flatMap((c) => c.subs.flatMap((s) => s.rows));
  const selectedRows = allRows.filter((r) => picked[r.key] && r.selectable);
  const countIn = (c: ReqCategory) =>
    c.subs.flatMap((s) => s.rows).filter((r) => picked[r.key] && r.selectable).length;
  const total = selectedRows.length;
  const countLabel = total === 0 ? "Nothing selected" : `${total} artifact${total > 1 ? "s" : ""}`;
  const scannedNote =
    assist.catalog_at !== null
      ? `Scanned on ${assist.owner_name}'s machine ${timeAgo(assist.catalog_at)} ago.`
      : `${assist.owner_name}'s engine has not scanned yet - it publishes while they view this assist.`;

  const addChip = () => {
    const p = fileDraft.trim();
    if (p !== "" && !chips.includes(p)) {
      setChips((c) => [...c, p]);
      setPicked((prev) => ({ ...prev, [`file:${p}`]: true }));
    }
    setFileDraft("");
  };

  const submit = () => {
    onSubmit(
      selectedRows.map((r) => ({
        kind: r.kind,
        target: r.target,
        payload: r.kind === "ssh" ? (myKey as string) : null,
      })),
      reason.trim(),
    );
  };

  return (
    <div className="overlay" onClick={onClose} style={{ padding: 30 }}>
      <div
        className="fade-in"
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "#fff",
          borderRadius: 14,
          boxShadow: "var(--shadow-lg)",
          width: "min(940px, 100%)",
          height: "min(620px, 88vh)",
          display: "grid",
          gridTemplateRows: "auto 1fr auto",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "15px 20px",
            borderBottom: "1px solid var(--color-neutral-200)",
          }}
        >
          <span style={{ fontSize: 15.5, fontWeight: 700 }}>Request artifacts</span>
          <span style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>
            Sent to {assist.owner_name} to approve - read-only
          </span>
          <button
            title="Close"
            aria-label="Close"
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              marginLeft: "auto",
              width: 28,
              height: 28,
              borderRadius: 7,
              display: "grid",
              placeItems: "center",
              color: "var(--color-neutral-600)",
            }}
            onClick={onClose}
          >
            <X size={14} />
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "212px 1fr", minHeight: 0 }}>
          <div
            style={{
              background: "var(--color-neutral-100)",
              borderRight: "1px solid var(--color-neutral-200)",
              padding: "12px 10px",
              display: "flex",
              flexDirection: "column",
              gap: 3,
            }}
          >
            {categories.map((c) => {
              const on = c.id === active.id;
              const n = countIn(c);
              const Icon = c.icon;
              return (
                <button
                  key={c.id}
                  onClick={() => setActiveId(c.id)}
                  style={{
                    border: "none",
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 9,
                    padding: "9px 11px",
                    borderRadius: 8,
                    background: on ? "#fff" : "transparent",
                    color: on ? "var(--color-text)" : "var(--color-neutral-700)",
                    boxShadow: on ? "var(--shadow-sm)" : "none",
                    font: "inherit",
                    fontSize: 13,
                    fontWeight: on ? 700 : 500,
                    textAlign: "left",
                  }}
                >
                  <Icon size={14} />
                  <span
                    style={{ flex: 1, minWidth: 0, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
                  >
                    {c.name}
                  </span>
                  {n > 0 && (
                    <span
                      style={{
                        minWidth: 18,
                        height: 18,
                        borderRadius: 9,
                        background: "var(--color-accent)",
                        color: "#fff",
                        display: "grid",
                        placeItems: "center",
                        fontSize: 10.5,
                        fontWeight: 700,
                        padding: "0 5px",
                      }}
                    >
                      {n}
                    </span>
                  )}
                </button>
              );
            })}
          </div>

          <div style={{ overflow: "auto", padding: "18px 22px" }}>
            <div style={{ fontSize: 14.5, fontWeight: 700 }}>{active.name}</div>
            <div style={{ fontSize: 12, color: "var(--color-neutral-600)", margin: "2px 0 16px" }}>
              {active.desc}
            </div>
            {active.subs.every((s) => s.rows.length === 0) && (
              <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: "0 0 12px" }}>
                {active.note}
              </p>
            )}
            {active.subs
              .filter((s) => s.rows.length > 0)
              .map((sub) => (
                <div key={sub.name} style={{ marginBottom: 18, maxWidth: 620 }}>
                  <div
                    style={{
                      fontSize: 11,
                      textTransform: "uppercase",
                      letterSpacing: "0.07em",
                      color: "var(--color-neutral-600)",
                      marginBottom: 7,
                    }}
                  >
                    {sub.name}
                  </div>
                  <div style={{ display: "grid", gap: 3 }}>
                    {sub.rows.map((row) => {
                      const on = !!picked[row.key] && row.selectable;
                      return (
                        <label
                          key={row.key}
                          title={row.granted ? "Already shared with you" : undefined}
                          style={{
                            position: "relative",
                            display: "flex",
                            gap: 11,
                            alignItems: "center",
                            padding: "8px 12px",
                            background: on ? "var(--color-accent-100)" : "var(--color-neutral-100)",
                            borderRadius: 8,
                            cursor: row.selectable ? "pointer" : "default",
                            boxShadow: on ? "inset 0 0 0 1.5px var(--color-accent)" : "none",
                          }}
                        >
                          <ReqIcon row={row} />
                          <span
                            style={{
                              fontSize: 13,
                              fontWeight: 600,
                              whiteSpace: "nowrap",
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              flexShrink: 0,
                            }}
                          >
                            {row.label}
                          </span>
                          <span
                            style={{
                              fontSize: 11.5,
                              color: "var(--color-neutral-500)",
                              whiteSpace: "nowrap",
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              flex: 1,
                              minWidth: 0,
                            }}
                          >
                            {row.detail}
                          </span>
                          {row.granted && (
                            <span className="tag tag-neutral" style={{ fontSize: 10 }}>
                              shared
                            </span>
                          )}
                          {row.selectable && (
                            <>
                              <span
                                style={{
                                  width: 17,
                                  height: 17,
                                  borderRadius: "50%",
                                  background: on ? "var(--color-accent)" : "#fff",
                                  boxShadow: on
                                    ? "none"
                                    : "inset 0 0 0 1.5px var(--color-neutral-300)",
                                  color: "#fff",
                                  display: "grid",
                                  placeItems: "center",
                                  flexShrink: 0,
                                  transition: "background .15s ease",
                                }}
                              >
                                {on && <Check size={10} strokeWidth={3.2} />}
                              </span>
                              <input
                                type="checkbox"
                                checked={on}
                                onChange={() =>
                                  setPicked((p) => ({ ...p, [row.key]: !p[row.key] }))
                                }
                                style={{ position: "absolute", opacity: 0, width: 0, height: 0 }}
                              />
                            </>
                          )}
                        </label>
                      );
                    })}
                  </div>
                </div>
              ))}

            {active.id === "ssh" && myCredentials.length > 0 && (
              <div style={{ marginBottom: 18, maxWidth: 620 }}>
                <div
                  style={{
                    fontSize: 11,
                    textTransform: "uppercase",
                    letterSpacing: "0.07em",
                    color: "var(--color-neutral-600)",
                    marginBottom: 7,
                  }}
                >
                  Provided access
                </div>
                <div style={{ display: "grid", gap: 3 }}>
                  {myCredentials.map((c) => (
                    <div
                      key={c.id}
                      style={{
                        display: "flex",
                        gap: 11,
                        alignItems: "center",
                        padding: "8px 12px",
                        background: "var(--color-neutral-100)",
                        borderRadius: 8,
                      }}
                    >
                      <TerminalSquare size={15} color="var(--color-neutral-600)" />
                      <span className="mono" style={{ fontSize: 12.5, flex: 1 }}>
                        {c.target}
                      </span>
                      {grantActive(c.id) ? (
                        <button
                          className="btn btn-primary"
                          style={{ padding: "4px 12px", fontSize: 12 }}
                          title="Open an SSH session in your terminal"
                          onClick={() =>
                            void openSsh(c.target as string).then((err) => {
                              if (err) onNotice(`Could not open an SSH terminal. ${err}`);
                            })
                          }
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

            {active.custom && (
              <div style={{ maxWidth: 620 }}>
                <div
                  style={{
                    fontSize: 11,
                    textTransform: "uppercase",
                    letterSpacing: "0.07em",
                    color: "var(--color-neutral-600)",
                    marginBottom: 8,
                  }}
                >
                  Enter file/directory paths
                </div>
                <div style={{ display: "flex", gap: 8, maxWidth: 420 }}>
                  <input
                    className="input mono"
                    style={{ fontSize: 12.5 }}
                    value={fileDraft}
                    onChange={(e) => setFileDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        addChip();
                      }
                    }}
                    placeholder="path/to/file or directory/"
                  />
                  <button
                    title="Add"
                    aria-label="Add path"
                    style={{
                      border: "none",
                      cursor: "pointer",
                      width: 36,
                      height: 36,
                      borderRadius: 8,
                      background: "var(--color-neutral-900)",
                      display: "grid",
                      placeItems: "center",
                      color: "#fff",
                      flexShrink: 0,
                    }}
                    onClick={addChip}
                  >
                    <Plus size={15} />
                  </button>
                </div>
              </div>
            )}

            <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: "14px 0 0" }}>
              {scannedNote}
            </p>
          </div>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 12,
            padding: "13px 20px",
            borderTop: "1px solid var(--color-neutral-200)",
          }}
        >
          <input
            className="input"
            style={{ maxWidth: 320, fontSize: 12.5 }}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            placeholder={`Reason, shown to ${assist.owner_name}`}
          />
          <span style={{ marginLeft: "auto", fontSize: 12.5, color: "var(--color-neutral-600)" }}>
            {countLabel} to request
          </span>
          <button
            className="btn btn-primary"
            disabled={busy || total === 0 || reason.trim() === ""}
            onClick={submit}
          >
            Request access
          </button>
        </div>
      </div>
    </div>
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
