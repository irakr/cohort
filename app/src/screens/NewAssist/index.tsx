import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Bot, Check, ChevronLeft, FileText, Folder, SquareTerminal, X } from "lucide-react";
import { envFingerprint, snapshotPaths, suggestArtifacts } from "../../api/agent";
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

function badgeColors(item: ArtifactCandidate): { bg: string; fg: string } {
  if (item.kind === "file") {
    return { bg: "var(--color-success-bg)", fg: "var(--color-success)" };
  }
  if (item.kind === "terminal") {
    return { bg: "var(--color-neutral-900)", fg: "#fff" };
  }
  if (item.badge === "CC") {
    return { bg: "#d97757", fg: "#fff" };
  }
  if (item.kind === "ai_agent") {
    return { bg: "var(--color-info)", fg: "#fff" };
  }
  return { bg: "var(--color-neutral-200)", fg: "var(--color-neutral-700)" };
}

/** Real app icon when the scan found one; folder/file glyphs for paths;
    otherwise the badge tile as placeholder. */
function ArtifactIcon({ item, size = 30 }: { item: ArtifactCandidate; size?: number }) {
  if (item.icon) {
    return (
      <img
        src={item.icon}
        alt=""
        width={size}
        height={size}
        style={{ borderRadius: 8, flexShrink: 0, objectFit: "contain" }}
      />
    );
  }
  if (item.kind === "file") {
    const isDirectory = !item.label.includes(".");
    const Glyph = isDirectory ? Folder : FileText;
    return (
      <IconTile size={size} bg="var(--color-success-bg)" fg="var(--color-success)">
        <Glyph size={Math.round(size * 0.5)} />
      </IconTile>
    );
  }
  const colors = badgeColors(item);
  return (
    <IconTile size={size} fontSize={Math.round(size / 3)} bg={colors.bg} fg={colors.fg}>
      {item.badge}
    </IconTile>
  );
}

function groupIcon(title: string) {
  if (title === "Files") {
    return <FileText size={13} />;
  }
  if (title === "AI agents") {
    return <Bot size={13} />;
  }
  return <SquareTerminal size={13} />;
}

export function NewAssist() {
  const { navigate } = useNav();
  // null = the agent module is still answering.
  const [suggested, setSuggested] = useState<ArtifactGroup[] | null>(null);
  const [customItems, setCustomItems] = useState<ArtifactCandidate[]>([]);
  const [picked, setPicked] = useState<Record<string, boolean>>({});
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState<Category | "">("");
  const [tags, setTags] = useState<string[]>([]);
  const [tagDraft, setTagDraft] = useState("");
  const [anonymous, setAnonymous] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [phase, setPhase] = useState<"select" | "analyzing">("select");
  const [analyzeStep, setAnalyzeStep] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);

  const [scanNonce, setScanNonce] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setSuggested(null);
    void suggestArtifacts().then((groups) => {
      if (!cancelled) {
        setSuggested(groups);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [scanNonce]);

  const allItems = useMemo(
    () => [...(suggested ?? []).flatMap((g) => g.items), ...customItems],
    [suggested, customItems],
  );
  const selected = allItems.filter((it) => picked[it.id]);
  const canCreate = title.trim().length > 0;

  function addTag(raw: string) {
    const tag = raw.trim().toLowerCase().replace(/\s+/g, "-");
    if (tag && !tags.includes(tag)) {
      setTags((t) => [...t, tag]);
    }
    setTagDraft("");
  }

  async function create() {
    setCreateError(null);
    setPhase("analyzing");
    const artifacts: AssistArtifact[] = selected.map((it) => ({
      id: it.id,
      kind: it.kind,
      label: it.label,
      detail: it.detail,
      icon: it.icon,
      pid: it.pid,
    }));
    try {
      setAnalyzeStep("Drafting insights from the selected artifacts...");
      const draft = await apiPost<BriefDraft>("/api/assists/draft-brief", {
        title: title.trim(),
        description: description.trim(),
        artifacts,
      });
      // Merge the machine's real fingerprint into the environment chips.
      const fingerprint = await envFingerprint();
      const environment = [
        ...draft.environment,
        ...fingerprint.filter((f) => !draft.environment.includes(f)),
      ];
      setAnalyzeStep("Creating the assist...");
      const detail = await apiPost<AssistDetail>("/api/assists", {
        title: title.trim(),
        tags,
        category: category || null,
        anonymous,
        description: description.trim(),
        insights: draft.insights,
        environment,
        artifacts,
      });
      // Upload a bounded, redacted snapshot of the shared files so the
      // responder's live view has real content. Failures are logged, not
      // fatal - the assist exists either way.
      const filePaths = artifacts.filter((a) => a.kind === "file").map((a) => a.detail);
      if (filePaths.length > 0) {
        setAnalyzeStep("Capturing shared files...");
        const snap = await snapshotPaths(filePaths);
        if (snap) {
          try {
            await apiPost(`/api/assists/${detail.ref}/artifacts`, {
              file_tree: snap.file_tree,
              files: snap.files,
              terminal_tabs: [],
              terminal_feed: [],
              agent_chat: [],
            });
          } catch (e) {
            console.error("live data upload failed:", e);
          }
        }
      }
      navigate({ name: "assist", ref: detail.ref });
    } catch (e) {
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
            {selected.length > 0
              ? `Analyzing ${selected.length} artifact${selected.length === 1 ? "" : "s"}`
              : "Creating the assist"}
          </h2>
          <p style={{ color: "var(--color-neutral-600)", fontSize: 13.5, margin: 0 }}>{analyzeStep}</p>
          <p style={{ color: "var(--color-neutral-500)", fontSize: 12, marginTop: 14 }}>
            Analyzed to draft the overview. Never shared with responders as-is.
          </p>
        </div>
      </div>
    );
  }

  const groups: ArtifactGroup[] = [
    ...(suggested ?? []),
    ...(customItems.length > 0 ? [{ title: "Added by you", items: customItems }] : []),
  ];
  const nothingToShow = suggested !== null && groups.every((g) => g.items.length === 0);

  return (
    <div style={{ maxWidth: 1100, margin: "0 auto", padding: "34px 28px" }}>
      <div style={{ display: "flex", alignItems: "flex-start", gap: 12, marginBottom: 4 }}>
        <h1 style={{ fontSize: 26, fontWeight: 700 }}>New assist</h1>
        <div style={{ flex: 1 }} />
        <button
          className="btn"
          style={{ padding: 7 }}
          title="Cancel and go back to assists"
          aria-label="Cancel"
          onClick={() => navigate({ name: "assists" })}
        >
          <X size={15} />
        </button>
      </div>
      <p style={{ color: "var(--color-neutral-600)", fontSize: 13.5, margin: "0 0 22px" }}>
        Suggested from what your agent module can see on this machine. Everything is
        read-only and starts unselected: nothing is shared until you toggle it on.
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 350px", gap: 22, alignItems: "start" }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
            <SectionTitle>Suggested artifacts</SectionTitle>
            <div style={{ flex: 1 }} />
            <button
              className="btn"
              title="Scan this machine again"
              disabled={suggested === null}
              onClick={() => setScanNonce((n) => n + 1)}
            >
              Rescan
            </button>
            <button className="btn btn-dark" onClick={() => setAddOpen(true)}>
              Add artifacts
            </button>
          </div>

          {suggested === null && (
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
              <Spinner size={14} /> Scanning this machine...
            </span>
          )}

          {nothingToShow && customItems.length === 0 && (
            <div className="card" style={{ padding: 20, fontSize: 13.5, color: "var(--color-neutral-600)" }}>
              Nothing detected right now: no interactive terminal sessions and no
              running or installed AI agents. Open a terminal in your project or
              start your agent and hit Rescan, or add artifacts manually.
            </div>
          )}

          {groups
            .filter((g) => g.items.length > 0)
            .map((group) => (
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
                  {groupIcon(group.title)}
                  {group.title}
                </div>
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
                ? "Analyzed to draft the overview. Never shared with responders as-is."
                : "Pick artifacts for Cohort to analyze. Optional."}
            </p>
          </div>

          <div className="card" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 14 }}>
            <div className="field">
              <label>Title: describe the problem, not yourself</label>
              <input
                className="input"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="One line on what is stuck"
              />
            </div>

            <div className="field">
              <label>What's happening: the error, what you tried</label>
              <textarea
                className="input"
                style={{ minHeight: 76, fontSize: 13.5 }}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Describe the problem in your own words. Markdown works."
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
              <label>Tags: stack and error specifics</label>
              <input
                className="input"
                value={tagDraft}
                onChange={(e) => setTagDraft(e.target.value)}
                placeholder="type a tag, press Enter"
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === ",") {
                    e.preventDefault();
                    addTag(tagDraft);
                  }
                }}
                onBlur={() => addTag(tagDraft)}
              />
              {tags.length > 0 && (
                <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                  {tags.map((t) => (
                    <button
                      key={t}
                      className="tag tag-neutral"
                      style={{ border: "none", cursor: "pointer" }}
                      title="Remove tag"
                      onClick={() => setTags((list) => list.filter((x) => x !== t))}
                    >
                      {t}
                      <X size={11} />
                    </button>
                  ))}
                </div>
              )}
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
          existingIds={allItems.map((it) => it.id)}
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
      <ArtifactIcon item={item} />
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

type AddKind = "file" | "terminal" | "ai_agent";

const ADD_META: Record<AddKind, { title: string; placeholder: string; icon: typeof FileText }> = {
  file: { title: "Add a file or directory", placeholder: "path/to/file or directory/", icon: FileText },
  terminal: { title: "Add a terminal", placeholder: "terminal name, e.g. iTerm2", icon: SquareTerminal },
  ai_agent: { title: "Add an AI agent", placeholder: "agent name, e.g. Claude Code", icon: Bot },
};

function badgeFor(kind: AddKind, value: string): string {
  if (kind === "terminal") {
    return ">_";
  }
  if (kind === "ai_agent") {
    return "AI";
  }
  const lower = value.toLowerCase();
  if (lower.endsWith(".yaml") || lower.endsWith(".yml")) {
    return "YML";
  }
  if (lower.endsWith(".log")) {
    return "LOG";
  }
  return "TXT";
}

function AddArtifactsModal({
  existingIds,
  onClose,
  onAdd,
}: {
  existingIds: string[];
  onClose: () => void;
  onAdd: (item: ArtifactCandidate) => void;
}) {
  const [step, setStep] = useState<"pick" | AddKind>("pick");
  const [value, setValue] = useState("");
  const [scanning, setScanning] = useState(false);
  const [found, setFound] = useState<ArtifactCandidate[]>([]);

  // Each step re-scans the machine so newly opened terminals, agent
  // sessions, and their directories show up without leaving the modal.
  useEffect(() => {
    if (step === "pick") {
      return;
    }
    let cancelled = false;
    setScanning(true);
    setFound([]);
    void suggestArtifacts().then((groups) => {
      if (cancelled) {
        return;
      }
      setFound(
        groups
          .flatMap((g) => g.items)
          .filter((it) => it.kind === step && !existingIds.includes(it.id)),
      );
      setScanning(false);
    });
    return () => {
      cancelled = true;
    };
  }, [step, existingIds]);

  const add = () => {
    const trimmed = value.trim();
    if (trimmed === "" || step === "pick") {
      return;
    }
    onAdd({
      id: `custom-${Date.now()}`,
      kind: step,
      badge: badgeFor(step, trimmed),
      label: step === "file" ? trimmed.split("/").filter(Boolean).pop() ?? trimmed : trimmed,
      detail: step === "file" ? trimmed : "added manually",
      warn: false,
      icon: null,
      pid: null,
    });
  };

  return (
    <Modal width={460} onClose={onClose}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 14 }}>
        {step !== "pick" && (
          <button
            className="btn"
            style={{ padding: 6 }}
            onClick={() => {
              setStep("pick");
              setValue("");
            }}
            title="Back"
          >
            <ChevronLeft size={15} />
          </button>
        )}
        <h3 style={{ fontSize: 16 }}>{step === "pick" ? "Add artifacts" : ADD_META[step].title}</h3>
      </div>

      {step === "pick" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {(Object.keys(ADD_META) as AddKind[]).map((kind) => {
            const Icon = ADD_META[kind].icon;
            return (
              <button
                key={kind}
                className="btn"
                style={{ justifyContent: "flex-start" }}
                onClick={() => setStep(kind)}
              >
                <Icon size={14} />
                {ADD_META[kind].title}
              </button>
            );
          })}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {scanning ? (
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
              <Spinner size={14} /> Scanning this machine...
            </span>
          ) : found.length > 0 ? (
            <div className="field">
              <label>Detected now</label>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {found.map((item) => (
                  <button
                    key={item.id}
                    className="btn"
                    style={{ justifyContent: "flex-start", gap: 10 }}
                    onClick={() => onAdd(item)}
                  >
                    <ArtifactIcon item={item} size={24} />
                    <span style={{ minWidth: 0, textAlign: "left" }}>
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
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: 0 }}>
              Nothing new detected for this type right now.
            </p>
          )}

          <div className="field">
            <label>Or add manually</label>
            <input
              className={step === "file" ? "input mono" : "input"}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder={ADD_META[step].placeholder}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  add();
                }
              }}
            />
          </div>
          <button className="btn btn-primary" disabled={!value.trim()} onClick={add}>
            Add read-only
          </button>
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: 0 }}>
            Selected artifacts are analyzed to draft the brief; access stays read-only.
          </p>
        </div>
      )}
    </Modal>
  );
}
