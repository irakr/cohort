import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Bot, Check, ChevronLeft, FileText, SquareTerminal, X } from "lucide-react";
import { suggestArtifacts } from "../../api/agent";
import { apiPost } from "../../api/client";
import type {
  ArtifactCandidate,
  ArtifactGroup,
  AssistArtifact,
  AssistDetail,
  BriefDraft,
  Category,
} from "../../api/types";
import { IconTile, Modal, SectionTitle, Spinner } from "../../components/ui";
import { CATEGORY_LABELS } from "../../util";
import { useNav } from "../../app/router";

const TAG_CHOICES = ["kubernetes", "helm", "registry-auth", "postgres", "ci", "networking"];
const GROUP_REVEAL_MS = [700, 1500, 2400];

const ANALYZE_STEPS = [
  "Fetching selected artifacts...",
  "Analyzing errors across sources...",
  "Creating the assist...",
];

function badgeColors(item: ArtifactCandidate): { bg: string; fg: string } {
  if (item.kind === "file") {
    return { bg: "var(--color-success-bg)", fg: "var(--color-success)" };
  }
  if (item.kind === "custom") {
    return { bg: "var(--color-neutral-200)", fg: "var(--color-neutral-700)" };
  }
  if (item.badge === "VS") {
    return { bg: "var(--color-info)", fg: "#fff" };
  }
  if (item.badge === "CC") {
    return { bg: "#d97757", fg: "#fff" };
  }
  if (item.badge === ">_") {
    return { bg: "#6b6763", fg: "#fff" };
  }
  return { bg: "var(--color-neutral-900)", fg: "#fff" };
}

export function NewAssist() {
  const { navigate } = useNav();
  const [groups, setGroups] = useState<ArtifactGroup[]>([]);
  const [revealed, setRevealed] = useState(0);
  const [customItems, setCustomItems] = useState<ArtifactCandidate[]>([]);
  const [picked, setPicked] = useState<Record<string, boolean>>({});
  const [title, setTitle] = useState("");
  const [category, setCategory] = useState<Category | "">("");
  const [tags, setTags] = useState<Record<string, boolean>>({});
  const [anonymous, setAnonymous] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [phase, setPhase] = useState<"select" | "analyzing">("select");
  const [stepIndex, setStepIndex] = useState(0);
  const [createError, setCreateError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void suggestArtifacts().then((g) => {
      if (!cancelled) {
        setGroups(g);
      }
    });
    const timers = GROUP_REVEAL_MS.map((ms, i) =>
      setTimeout(() => setRevealed((r) => Math.max(r, i + 1)), ms),
    );
    return () => {
      cancelled = true;
      timers.forEach(clearTimeout);
    };
  }, []);

  const allItems = useMemo(
    () => [...groups.flatMap((g) => g.items), ...customItems],
    [groups, customItems],
  );
  const selected = allItems.filter((it) => picked[it.id]);
  const canCreate = title.trim().length > 0;

  async function create() {
    setCreateError(null);
    setPhase("analyzing");
    setStepIndex(0);
    const stepTimers = [
      setTimeout(() => setStepIndex(1), 1100),
      setTimeout(() => setStepIndex(2), 2200),
    ];
    const artifacts: AssistArtifact[] = selected.map((it) => ({
      id: it.id,
      kind: it.kind,
      label: it.label,
      detail: it.detail,
    }));
    try {
      const draft = await apiPost<BriefDraft>("/api/assists/draft-brief", {
        title: title.trim(),
        artifacts,
      });
      const detail = await apiPost<AssistDetail>("/api/assists", {
        title: title.trim(),
        tags: Object.keys(tags).filter((t) => tags[t]),
        category: category || null,
        anonymous,
        goal: draft.goal,
        failures: draft.failures,
        environment: draft.environment,
        artifacts,
      });
      navigate({ name: "assist", ref: detail.ref });
    } catch (e) {
      stepTimers.forEach(clearTimeout);
      setPhase("select");
      setCreateError(e instanceof Error ? e.message : String(e));
    }
  }

  if (phase === "analyzing") {
    return (
      <div style={{ display: "grid", placeItems: "center", minHeight: "100vh" }}>
        <div className="card fade-in" style={{ width: 520, padding: 36, textAlign: "center" }}>
          <Spinner size={26} />
          <h2 style={{ fontSize: 19, margin: "14px 0 6px" }}>
            Analyzing {selected.length} artifact{selected.length === 1 ? "" : "s"}
          </h2>
          <p style={{ color: "var(--color-neutral-600)", fontSize: 13.5, margin: 0 }}>
            {ANALYZE_STEPS[stepIndex]}
          </p>
          <p style={{ color: "var(--color-neutral-500)", fontSize: 12, marginTop: 14 }}>
            Analyzed locally and on your hub to draft the overview. Never shared with responders.
          </p>
        </div>
      </div>
    );
  }

  const visibleGroups: (ArtifactGroup & { loading: boolean })[] = [
    ...groups.map((g, i) => ({ ...g, loading: i >= revealed })),
    ...(customItems.length > 0 ? [{ title: "Custom", items: customItems, loading: false }] : []),
  ];

  return (
    <div style={{ maxWidth: 1100, margin: "0 auto", padding: "34px 28px" }}>
      <h1 style={{ fontSize: 26, fontWeight: 700, marginBottom: 4 }}>New assist</h1>
      <p style={{ color: "var(--color-neutral-600)", fontSize: 13.5, margin: "0 0 22px" }}>
        Suggested from what your agent module already sees. Everything is read-only and
        starts unselected: nothing is shared until you toggle it on.
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 350px", gap: 22, alignItems: "start" }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", marginBottom: 10 }}>
            <SectionTitle>Suggested artifacts</SectionTitle>
            <div style={{ flex: 1 }} />
            <button className="btn btn-dark" onClick={() => setAddOpen(true)}>
              Add artifacts
            </button>
          </div>

          {visibleGroups.map((group) => (
            <div key={group.title} style={{ marginBottom: 18 }}>
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 7,
                  fontSize: 12.5,
                  fontWeight: 700,
                  color: "var(--color-neutral-700)",
                  marginBottom: 8,
                }}
              >
                {group.title === "Files" ? (
                  <FileText size={13} />
                ) : group.title === "AI agents" ? (
                  <Bot size={13} />
                ) : (
                  <SquareTerminal size={13} />
                )}
                {group.title}
                {group.loading && (
                  <span style={{ display: "inline-flex", alignItems: "center", gap: 5, color: "var(--color-neutral-500)", fontWeight: 400 }}>
                    <Spinner size={12} /> fetching...
                  </span>
                )}
              </div>
              {!group.loading && (
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {group.items.map((item) => (
                    <ArtifactCard
                      key={item.id}
                      item={item}
                      checked={!!picked[item.id]}
                      toggle={() => setPicked((p) => ({ ...p, [item.id]: !p[item.id] }))}
                    />
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>

        <div style={{ position: "sticky", top: 24, display: "flex", flexDirection: "column", gap: 14 }}>
          <div className="card" style={{ padding: 16 }}>
            <div style={{ fontSize: 13, fontWeight: 700, marginBottom: 8 }}>
              For analysis - {selected.length === 0 ? "nothing selected" : `${selected.length} item${selected.length === 1 ? "" : "s"}`}
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
              {selected.map((it) => (
                <button
                  key={it.id}
                  className="tag tag-accent"
                  style={{ border: "none", cursor: "pointer" }}
                  onClick={() => setPicked((p) => ({ ...p, [it.id]: false }))}
                  title="Remove"
                >
                  {it.label}
                  <X size={11} />
                </button>
              ))}
            </div>
            <p style={{ fontSize: 12, color: "var(--color-neutral-600)", margin: 0 }}>
              {selected.length > 0
                ? "Analyzed to draft the overview. Never shared with responders."
                : "Pick artifacts for Cohort to analyze."}
            </p>
          </div>

          <div className="card" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 14 }}>
            <div className="field">
              <label>Title: describe the problem, not yourself</label>
              <input
                className="input"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="Rollout stuck on image pull"
              />
            </div>

            <div className="field">
              <label>Category (optional): what kind of help</label>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {(Object.keys(CATEGORY_LABELS) as Category[]).map((c) => (
                  <button
                    key={c}
                    className={`btn${category === c ? " btn-on" : ""}`}
                    style={{ padding: "5px 10px", fontSize: 12 }}
                    onClick={() => setCategory(category === c ? "" : c)}
                  >
                    {CATEGORY_LABELS[c]}
                  </button>
                ))}
              </div>
            </div>

            <div className="field">
              <label>Tags</label>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {TAG_CHOICES.map((t) => (
                  <button
                    key={t}
                    className={`btn${tags[t] ? " btn-on" : ""}`}
                    style={{ padding: "5px 10px", fontSize: 12 }}
                    onClick={() => setTags((prev) => ({ ...prev, [t]: !prev[t] }))}
                  >
                    {t}
                  </button>
                ))}
              </div>
            </div>

            <label
              onClick={() => setAnonymous((a) => !a)}
              style={{ display: "flex", alignItems: "center", gap: 10, cursor: "pointer", fontSize: 13 }}
            >
              <span
                style={{
                  position: "relative",
                  width: 32,
                  height: 18,
                  borderRadius: 9,
                  background: anonymous ? "var(--color-accent)" : "var(--color-neutral-300)",
                  transition: "background .15s ease",
                  flexShrink: 0,
                }}
              >
                <span
                  style={{
                    position: "absolute",
                    top: 2,
                    left: anonymous ? 16 : 2,
                    width: 14,
                    height: 14,
                    borderRadius: "50%",
                    background: "#fff",
                    transition: "left .15s ease",
                  }}
                />
              </span>
              Post without my name
            </label>

            {createError && (
              <div style={{ fontSize: 12.5, color: "var(--color-accent-700)" }}>{createError}</div>
            )}
            <button className="btn btn-primary btn-block" disabled={!canCreate} onClick={() => void create()}>
              Create assist
            </button>
          </div>
        </div>
      </div>

      {addOpen && (
        <AddArtifactsModal
          onClose={() => setAddOpen(false)}
          onAdd={(item) => {
            setCustomItems((items) => [...items, item]);
            setPicked((p) => ({ ...p, [item.id]: true }));
            setAddOpen(false);
          }}
        />
      )}
    </div>
  );
}

function ArtifactCard({
  item,
  checked,
  toggle,
}: {
  item: ArtifactCandidate;
  checked: boolean;
  toggle: () => void;
}) {
  const colors = badgeColors(item);
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "10px 12px",
        borderRadius: "var(--radius-card)",
        background: checked ? "var(--color-accent-100)" : "#fff",
        boxShadow: checked ? "inset 0 0 0 2px var(--color-accent)" : "var(--shadow-sm)",
        cursor: "pointer",
      }}
    >
      <input type="checkbox" checked={checked} onChange={toggle} style={{ display: "none" }} />
      <IconTile bg={colors.bg} fg={colors.fg}>
        {item.badge}
      </IconTile>
      <span style={{ minWidth: 0, flex: 1 }}>
        <span style={{ display: "block", fontSize: 13.5, fontWeight: 600 }}>{item.label}</span>
        <span style={{ display: "block", fontSize: 12, color: "var(--color-neutral-600)" }}>
          {item.detail}
        </span>
      </span>
      {item.warn && (
        <AlertTriangle
          size={15}
          color="var(--color-warning-fg)"
          aria-label="may expose sensitive content"
        />
      )}
      <span
        style={{
          display: "grid",
          placeItems: "center",
          width: 20,
          height: 20,
          borderRadius: "50%",
          background: checked ? "var(--color-accent)" : "var(--color-neutral-200)",
          color: "#fff",
          flexShrink: 0,
        }}
      >
        {checked && <Check size={13} />}
      </span>
    </label>
  );
}

const SUGGESTED_PATHS = ["k8s/secrets/regcred.yaml", ".github/workflows/deploy.yml"];

function AddArtifactsModal({
  onClose,
  onAdd,
}: {
  onClose: () => void;
  onAdd: (item: ArtifactCandidate) => void;
}) {
  const [step, setStep] = useState<"pick" | "files" | "term" | "agents">("pick");
  const [path, setPath] = useState("");
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    if (step === "term" || step === "agents") {
      setScanning(true);
      const t = setTimeout(() => setScanning(false), 1200);
      return () => clearTimeout(t);
    }
  }, [step]);

  const addFile = (p: string) => {
    const trimmed = p.trim();
    if (!trimmed) {
      return;
    }
    onAdd({
      id: `custom-${Date.now()}`,
      kind: "custom",
      badge: "TXT",
      label: trimmed.split("/").pop() ?? trimmed,
      detail: trimmed,
      warn: false,
    });
  };

  const titles: Record<string, string> = {
    pick: "Add artifacts",
    files: "Add a file or directory",
    term: "Attach a terminal",
    agents: "Attach an AI agent",
  };

  return (
    <Modal width={460} onClose={onClose}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 14 }}>
        {step !== "pick" && (
          <button
            className="btn"
            style={{ padding: 6 }}
            onClick={() => setStep("pick")}
            title="Back"
          >
            <ChevronLeft size={15} />
          </button>
        )}
        <h3 style={{ fontSize: 16 }}>{titles[step]}</h3>
      </div>

      {step === "pick" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <button className="btn" style={{ justifyContent: "flex-start" }} onClick={() => setStep("files")}>
            <FileText size={14} /> Files and directories
          </button>
          <button className="btn" style={{ justifyContent: "flex-start" }} onClick={() => setStep("term")}>
            <SquareTerminal size={14} /> Terminals
          </button>
          <button className="btn" style={{ justifyContent: "flex-start" }} onClick={() => setStep("agents")}>
            <Bot size={14} /> AI agents
          </button>
        </div>
      )}

      {step === "files" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <input
            className="input mono"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="path/to/file or directory/"
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                addFile(path);
              }
            }}
          />
          <div style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>Suggested</div>
          {SUGGESTED_PATHS.map((p) => (
            <button
              key={p}
              className="btn mono"
              style={{ justifyContent: "flex-start", fontSize: 12 }}
              onClick={() => addFile(p)}
            >
              {p}
            </button>
          ))}
          <button className="btn btn-primary" disabled={!path.trim()} onClick={() => addFile(path)}>
            Add read-only
          </button>
        </div>
      )}

      {(step === "term" || step === "agents") && (
        <div style={{ display: "flex", flexDirection: "column", gap: 10, alignItems: "flex-start" }}>
          {scanning ? (
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
              <Spinner size={14} />
              {step === "term"
                ? "Listing active terminal sessions on this machine..."
                : "Listing agent sessions on this machine..."}
            </span>
          ) : (
            <p style={{ fontSize: 13, color: "var(--color-neutral-600)", margin: 0 }}>
              Everything the agent module detected is already in the suggestions list.
            </p>
          )}
        </div>
      )}
    </Modal>
  );
}
