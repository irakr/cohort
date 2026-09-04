import { useEffect, useMemo, useState } from "react";
import { Clock, User as UserIcon, Users } from "lucide-react";
import { useApi } from "../../api/hooks";
import type { AssistStatus, AssistSummary } from "../../api/types";
import { Spinner, STATUS_META, StatusDot } from "../../components/ui";
import { timeAgo } from "../../util";
import { useNav } from "../../app/router";

export function OpenAssists() {
  const { navigate } = useNav();
  const [statusFilter, setStatusFilter] = useState<AssistStatus | "all">("all");
  const [tagFilter, setTagFilter] = useState("All");
  const [mineOnly, setMineOnly] = useState(false);

  const path = useMemo(() => {
    const params = new URLSearchParams();
    if (statusFilter !== "all") {
      params.set("status", statusFilter);
    }
    if (tagFilter !== "All") {
      params.set("tag", tagFilter);
    }
    if (mineOnly) {
      params.set("mine", "true");
    }
    const qs = params.toString();
    return `/api/assists${qs ? `?${qs}` : ""}`;
  }, [statusFilter, tagFilter, mineOnly]);

  const { data: assists, loading, error } = useApi<AssistSummary[]>(path, { pollMs: 15000 });
  // Unfiltered list for the status chip counts.
  const { data: allAssists } = useApi<AssistSummary[]>("/api/assists", { pollMs: 15000 });

  // Tag chips are whatever the assists actually carry. A fresh hub has none,
  // and a tag disappears from the row once the last assist using it is gone.
  const tagFilters = useMemo(() => {
    const tags = new Set<string>();
    for (const a of allAssists ?? []) {
      for (const t of a.tags) {
        tags.add(t);
      }
    }
    return [...tags].sort();
  }, [allAssists]);

  useEffect(() => {
    if (tagFilter !== "All" && !tagFilters.includes(tagFilter)) {
      setTagFilter("All");
    }
  }, [tagFilter, tagFilters]);

  return (
    <div style={{ maxWidth: 900, margin: "0 auto", padding: "34px 28px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 18 }}>
        <h1 style={{ fontSize: 26, fontWeight: 700 }}>Assists</h1>
        <div style={{ flex: 1 }} />
        {(["open", "dormant", "done"] as AssistStatus[]).map((s) => {
          const active = statusFilter === s;
          const count = allAssists?.filter((a) => a.status === s).length ?? 0;
          return (
            <button
              key={s}
              title={`${STATUS_META[s].tip} - click to filter`}
              onClick={() => setStatusFilter(active ? "all" : s)}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 7,
                border: "none",
                cursor: "pointer",
                borderRadius: 20,
                padding: "5px 12px",
                fontSize: 12.5,
                fontWeight: 600,
                background: active ? "var(--color-neutral-900)" : "#fff",
                color: active ? "#fff" : "var(--color-text)",
                boxShadow: "var(--shadow-sm)",
              }}
            >
              <StatusDot status={s} size={8} />
              {count} {STATUS_META[s].label}
            </button>
          );
        })}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 16 }}>
        <button
          className={`btn${mineOnly ? " btn-on" : ""}`}
          onClick={() => setMineOnly((m) => !m)}
          title="Assists you own or respond to"
        >
          <UserIcon size={13} />
          My assists
        </button>
        {tagFilters.length > 0 && (
          <>
            <div style={{ width: 1, height: 22, background: "var(--color-neutral-300)" }} />
            {["All", ...tagFilters].map((t) => (
              <button
                key={t}
                className={`btn${tagFilter === t ? " btn-on" : ""}`}
                onClick={() => setTagFilter(t)}
              >
                {t}
              </button>
            ))}
          </>
        )}
      </div>

      {loading && (
        <div style={{ display: "flex", justifyContent: "center", padding: 40 }}>
          <Spinner size={22} />
        </div>
      )}
      {error && (
        <div className="card" style={{ padding: 18, color: "var(--color-accent-700)" }}>
          Could not load assists: {error}. If the hub is the wrong one, change its
          URL in the rail settings.
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {assists?.map((a) => (
          <AssistCard key={a.ref} assist={a} open={() => navigate({ name: "assist", ref: a.ref })} />
        ))}
        {assists && assists.length === 0 && !loading && (
          <div className="card" style={{ padding: 24, color: "var(--color-neutral-600)", fontSize: 14 }}>
            No assists match these filters.
          </div>
        )}
      </div>
    </div>
  );
}

function AssistCard({ assist, open }: { assist: AssistSummary; open: () => void }) {
  const hasResponders = assist.responder_names.length > 0;
  return (
    <button
      onClick={open}
      className="card"
      style={{
        display: "grid",
        gridTemplateColumns: "1fr 190px",
        gap: 12,
        padding: "16px 18px",
        border: "none",
        textAlign: "left",
        cursor: "pointer",
        font: "inherit",
        color: "inherit",
        transition: "box-shadow .15s ease, transform .15s ease",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.boxShadow = "var(--shadow-md)";
        e.currentTarget.style.transform = "translateY(-1px)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.boxShadow = "var(--shadow-sm)";
        e.currentTarget.style.transform = "none";
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
          <StatusDot status={assist.status} />
          <span style={{ fontSize: 11.5, fontWeight: 700, color: "var(--color-neutral-600)" }}>
            {assist.ref}
          </span>
        </div>
        <div style={{ fontSize: 16, fontWeight: 600, marginBottom: 8 }}>{assist.title}</div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {assist.tags.map((t) => (
            <span key={t} className="tag tag-neutral">
              {t}
            </span>
          ))}
        </div>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 6,
          fontSize: 12.5,
          color: "var(--color-neutral-600)",
          justifyContent: "center",
        }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <UserIcon size={13} />
          {assist.owner_name}
        </span>
        <span
          title={hasResponders ? `Responding: ${assist.responder_names.join(", ")}` : "No responder yet"}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            color: hasResponders ? "var(--color-accent-700)" : "var(--color-neutral-400)",
          }}
        >
          <Users size={13} />
          {hasResponders ? assist.responder_names.join(", ") : "no responder yet"}
        </span>
        <span className="mono" style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11 }}>
          <Clock size={13} />
          {timeAgo(assist.created_at)}
        </span>
      </div>
    </button>
  );
}
