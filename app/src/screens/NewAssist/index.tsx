import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Bot, Check, FileText, Folder, Plus, SquareTerminal, X } from "lucide-react";
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
import { IconTile, Spinner } from "../../components/ui";
import { CATEGORY_LABELS } from "../../util";
import { useNav } from "../../app/router";

function glyphFor(item: ArtifactCandidate): { Glyph: typeof FileText; color: string } {
  if (item.kind === "terminal") {
    return { Glyph: SquareTerminal, color: "var(--color-neutral-600)" };
  }
  if (item.kind === "ai_agent") {
    return { Glyph: Bot, color: item.badge === "CC" ? "#d97757" : "var(--color-neutral-600)" };
  }
  const isDirectory = item.detail.endsWith("/") || !item.label.includes(".");
  return isDirectory
    ? { Glyph: Folder, color: "#64a8e8" }
    : { Glyph: FileText, color: "var(--color-neutral-600)" };
}

/** Real app icon when the scan found one; monochrome stroke glyph otherwise. */
function ArtifactIcon({ item, size = 15 }: { item: ArtifactCandidate; size?: number }) {
  if (item.icon) {
    return (
      <img
        src={item.icon}
        alt=""
        width={size + 4}
        height={size + 4}
        style={{ borderRadius: 5, flexShrink: 0, objectFit: "contain" }}
      />
    );
  }
  const { Glyph, color } = glyphFor(item);
  return <Glyph size={size} color={color} style={{ flexShrink: 0 }} />;
}

/** Page-card variant: the glyph/app icon inside a 28px neutral tile. */
function ArtifactTile({ item }: { item: ArtifactCandidate }) {
  return (
    <IconTile size={28} radius={7} bg="var(--color-neutral-200)" fg="var(--color-neutral-700)">
      <ArtifactIcon item={item} size={14} />
    </IconTile>
  );
}

interface ArtifactCategory {
  id: string;
  name: string;
  icon: typeof FileText;
  desc: string;
  subs: { name: string; items: ArtifactCandidate[] }[];
  /** Shows the custom path input (Files tab). */
  custom?: boolean;
  /** Honest note shown when the category has nothing detectable yet. */
  note?: string;
}

function buildCategories(
  suggested: ArtifactGroup[],
  customItems: ArtifactCandidate[],
): ArtifactCategory[] {
  const itemsOf = (title: string) =>
    suggested.find((g) => g.title === title)?.items ?? [];
  const agents = itemsOf("AI agents");
  const isActive = (a: ArtifactCandidate) =>
    a.detail.includes("active") || a.detail.startsWith("running");
  return [
    {
      id: "term",
      name: "Terminals",
      icon: SquareTerminal,
      desc: "Selected terminal sessions will be read by the Cohort agent to analyze what you require.",
      subs: [{ name: "Detected sessions", items: itemsOf("Terminals") }],
      note: "No terminal sessions detected right now.",
    },
    {
      id: "files",
      name: "Files & Directories",
      icon: FileText,
      desc: "Selected files and directories will be read by the Cohort agent to analyze what you require.",
      custom: true,
      subs: [
        { name: "Suggested", items: itemsOf("Files") },
        { name: "Added by you", items: customItems },
      ],
      note: "Nothing detected - add a path below.",
    },
    {
      id: "agent",
      name: "AI agents",
      icon: Bot,
      desc: "Selected agent instances will be read by the Cohort agent to analyze what you require.",
      subs: [
        { name: "Active", items: agents.filter(isActive) },
        { name: "Installed", items: agents.filter((a) => !isActive(a)) },
      ],
      note: "No AI agents detected on this machine.",
    },
  ];
}

export function NewAssist() {
  const { navigate } = useNav();
  // null = the agent module is still answering.
  const [suggested, setSuggested] = useState<ArtifactGroup[] | null>(null);
  const [fingerprint, setFingerprint] = useState<string[]>([]);
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

  // Scan on mount and again whenever the Add artifacts dialog opens, so the
  // dialog always shows the machine's current state.
  const [scanNonce, setScanNonce] = useState(0);
  useEffect(() => {
    let cancelled = false;
    void Promise.all([suggestArtifacts(), envFingerprint()]).then(([groups, fp]) => {
      if (!cancelled) {
        setSuggested(groups);
        setFingerprint(fp);
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
  const countLabel = selected.length > 0 ? `${selected.length} selected` : "None selected";

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
        <div
          className="fade-in"
          style={{
            width: 520,
            background: "#fff",
            borderRadius: 14,
            boxShadow: "var(--shadow-md)",
            padding: "34px 36px",
            textAlign: "center",
          }}
        >
          <Spinner size={26} />
          <h2 style={{ fontSize: 16, fontWeight: 700, margin: "14px 0 0" }}>
            {selected.length > 0
              ? `Analyzing ${selected.length} artifact${selected.length === 1 ? "" : "s"}`
              : "Creating the assist"}
          </h2>
          <p style={{ color: "var(--color-neutral-600)", fontSize: 12.5, margin: "6px 0 0" }}>
            {analyzeStep}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div style={{ padding: "30px 38px 80px" }} className="fade-in">
      <div style={{ display: "grid", gridTemplateColumns: "1fr auto 1fr", alignItems: "center" }}>
        <span />
        <h1 style={{ fontSize: 26, fontWeight: 700, textAlign: "center" }}>New assist</h1>
        <button
          className="btn"
          style={{ padding: 7, justifySelf: "end" }}
          title="Cancel and go back to assists"
          aria-label="Cancel"
          onClick={() => navigate({ name: "assists" })}
        >
          <X size={15} />
        </button>
      </div>

      <div style={{ maxWidth: 680, margin: "26px auto 0", display: "grid", gap: 16 }}>
        <div className="card" style={{ padding: "20px 22px", display: "grid", gap: 14 }}>
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
                  style={{ padding: "3px 10px", fontSize: 11 }}
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
            style={{
              display: "flex",
              gap: 10,
              cursor: "pointer",
              fontSize: 13,
              alignItems: "center",
              justifyContent: "space-between",
            }}
          >
            <span
              style={{ fontWeight: 600 }}
              title="Listed as a teammate. Responders still reach you inside the assist"
            >
              Anonymous
            </span>
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
                  boxShadow: "var(--shadow-sm)",
                  transition: "left .15s ease",
                }}
              />
            </span>
          </label>
        </div>

        <div className="card" style={{ padding: "20px 22px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <div style={{ fontSize: 15, fontWeight: 700 }}>Artifacts - {countLabel}</div>
            <span style={{ marginLeft: "auto" }} />
            <button
              className="btn btn-dark"
              style={{ fontSize: 12.5 }}
              onClick={() => {
                setScanNonce((n) => n + 1);
                setAddOpen(true);
              }}
            >
              <Plus size={13} />
              Add artifacts
            </button>
          </div>
          <div style={{ fontSize: 12, color: "var(--color-neutral-600)", margin: "3px 0 12px" }}>
            {selected.length > 0
              ? "Analyzed to draft the overview. Never shared with responders as-is."
              : "Nothing selected. Pick artifacts for Cohort to analyze."}
          </div>
          {selected.length > 0 ? (
            <div style={{ display: "grid", gap: 6 }}>
              {selected.map((it) => (
                <div
                  key={it.id}
                  style={{
                    display: "flex",
                    gap: 10,
                    alignItems: "center",
                    padding: "8px 10px",
                    background: "var(--color-neutral-100)",
                    borderRadius: 9,
                  }}
                >
                  <ArtifactTile item={it} />
                  <span style={{ minWidth: 0, flex: 1 }}>
                    <span
                      style={{
                        display: "block",
                        fontSize: 12.5,
                        fontWeight: 600,
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                    >
                      {it.label}
                    </span>
                    <span
                      style={{
                        display: "block",
                        fontSize: 11,
                        color: "var(--color-neutral-600)",
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                    >
                      {it.detail}
                    </span>
                  </span>
                  <button
                    title="Remove"
                    style={{
                      border: "none",
                      background: "transparent",
                      cursor: "pointer",
                      width: 24,
                      height: 24,
                      borderRadius: 6,
                      display: "grid",
                      placeItems: "center",
                      color: "var(--color-neutral-600)",
                      flexShrink: 0,
                    }}
                    onClick={() => setPicked((p) => ({ ...p, [it.id]: false }))}
                  >
                    <X size={13} />
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div
              style={{
                border: "1.5px dashed var(--color-neutral-300)",
                borderRadius: 10,
                padding: 18,
                textAlign: "center",
                fontSize: 12.5,
                color: "var(--color-neutral-600)",
              }}
            >
              No artifacts yet. Add terminals, files, agents or app windows for Cohort to analyze.
            </div>
          )}
        </div>

        {createError && (
          <div style={{ fontSize: 12.5, color: "var(--color-accent-700)" }}>{createError}</div>
        )}
        <button className="btn btn-primary btn-block" disabled={!canCreate} onClick={() => void create()}>
          Create assist
        </button>
      </div>

      {addOpen && (
        <AddArtifactsModal
          categories={buildCategories(suggested ?? [], customItems)}
          scanning={suggested === null}
          picked={picked}
          selectedCount={countLabel}
          onToggle={(id) => setPicked((p) => ({ ...p, [id]: !p[id] }))}
          onAddCustom={(path) => {
            const item: ArtifactCandidate = {
              id: `custom-${Date.now()}`,
              kind: "file",
              badge: path.toLowerCase().match(/\.(ya?ml)$/) ? "YML" : "TXT",
              label: path.split("/").filter(Boolean).pop() ?? path,
              detail: path,
              warn: false,
              icon: null,
              pid: null,
            };
            setCustomItems((items) => [...items, item]);
            setPicked((p) => ({ ...p, [item.id]: true }));
          }}
          onClose={() => setAddOpen(false)}
        />
      )}
    </div>
  );
}

// The redesigned Add artifacts dialog: category tabs on the left, selectable
// artifact cards (grouped) on the right, all sourced from the live scan.
function AddArtifactsModal({
  categories,
  scanning,
  picked,
  selectedCount,
  onToggle,
  onAddCustom,
  onClose,
}: {
  categories: ArtifactCategory[];
  scanning: boolean;
  picked: Record<string, boolean>;
  selectedCount: string;
  onToggle: (id: string) => void;
  onAddCustom: (path: string) => void;
  onClose: () => void;
}) {
  const [activeId, setActiveId] = useState(categories[0]?.id ?? "term");
  const [customDraft, setCustomDraft] = useState("");
  const active = categories.find((c) => c.id === activeId) ?? categories[0];
  const countIn = (c: ArtifactCategory) =>
    c.subs.flatMap((s) => s.items).filter((it) => picked[it.id]).length;
  const visibleSubs = active.subs.filter((s) => s.items.length > 0);

  const addCustom = () => {
    const path = customDraft.trim();
    if (path !== "") {
      onAddCustom(path);
      setCustomDraft("");
    }
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
          <span style={{ fontSize: 15.5, fontWeight: 700 }}>Add artifacts</span>
          <span style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>
            Read-only - analyzed locally, never shared with responders
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
            {scanning ? (
              <span style={{ display: "inline-flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--color-neutral-600)" }}>
                <Spinner size={14} /> Scanning this machine...
              </span>
            ) : (
              <>
                {visibleSubs.length === 0 && (
                  <p style={{ fontSize: 12.5, color: "var(--color-neutral-600)", margin: 0 }}>
                    {active.note}
                  </p>
                )}
                {visibleSubs.map((sub) => (
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
                      {sub.items.map((item) => {
                        const on = !!picked[item.id];
                        return (
                          <label
                            key={item.id}
                            style={{
                              position: "relative",
                              display: "flex",
                              gap: 11,
                              alignItems: "center",
                              padding: "8px 12px",
                              background: on ? "var(--color-accent-100)" : "var(--color-neutral-100)",
                              borderRadius: 8,
                              cursor: "pointer",
                              boxShadow: on ? "inset 0 0 0 1.5px var(--color-accent)" : "none",
                            }}
                          >
                            <ArtifactIcon item={item} />
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
                              {item.label}
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
                              {item.detail}
                            </span>
                            {item.warn && (
                              <AlertTriangle
                                size={14}
                                color="var(--color-warning-fg)"
                                aria-label="may not work in this environment"
                              />
                            )}
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
                              onChange={() => onToggle(item.id)}
                              style={{ position: "absolute", opacity: 0, width: 0, height: 0 }}
                            />
                          </label>
                        );
                      })}
                    </div>
                  </div>
                ))}
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
                        value={customDraft}
                        onChange={(e) => setCustomDraft(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            addCustom();
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
                        onClick={addCustom}
                      >
                        <Plus size={15} />
                      </button>
                    </div>
                  </div>
                )}
              </>
            )}
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
          <span style={{ fontSize: 12.5, color: "var(--color-neutral-600)" }}>
            {selectedCount} for analysis
          </span>
          <span style={{ marginLeft: "auto" }} />
          <button className="btn btn-primary" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
