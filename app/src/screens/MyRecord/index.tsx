import { useState } from "react";
import { Award, Bot, FileText, Lock, Reply, Share } from "lucide-react";
import { useApi } from "../../api/hooks";
import type { MyRecord as MyRecordT } from "../../api/types";
import { AvatarChip, IconTile, SectionTitle, Spinner, StatusDot } from "../../components/ui";
import { OUTCOME_LABELS } from "../../util";
import { useNav } from "../../app/router";

export function MyRecord() {
  const { navigate } = useNav();
  const { data: record, loading, error } = useApi<MyRecordT>("/api/my-record");
  const [range, setRange] = useState("30d");

  if (loading) {
    return (
      <div style={{ display: "grid", placeItems: "center", minHeight: "60vh" }}>
        <Spinner size={22} />
      </div>
    );
  }
  if (error || !record) {
    return (
      <div style={{ maxWidth: 900, margin: "0 auto", padding: "34px 28px" }}>
        <div className="card" style={{ padding: 18, color: "var(--color-accent-700)" }}>
          {error ?? "Could not load your record"}
        </div>
      </div>
    );
  }

  const usage = record.ai_usage.find((u) => u.range === range) ?? record.ai_usage[0];

  const stats = [
    { label: "Credits", value: record.credits_earned, icon: Award, tip: "Credited by owners you helped" },
    { label: "Responses", value: record.responses_count, icon: Reply, tip: "Assists you responded to" },
    { label: "Records", value: record.records_count, icon: FileText, tip: "Resolution records you authored" },
  ];

  return (
    <div style={{ maxWidth: 940, margin: "0 auto", padding: "34px 28px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 20 }}>
        <h1 style={{ fontSize: 26, fontWeight: 700 }}>{record.user.name}</h1>
        <span
          className="tag tag-neutral"
          title="Private. Only you can see this."
          style={{ display: "inline-flex", gap: 5 }}
        >
          <Lock size={11} /> private
        </span>
        <div style={{ flex: 1 }} />
        <button
          className="btn btn-secondary"
          title="Sharing is an explicit export you choose; never a standing permission"
          onClick={() => alert("Export arrives in a later phase. Nothing is shared until you do this.")}
        >
          <Share size={13} /> Share
        </button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14, marginBottom: 26 }}>
        {stats.map(({ label, value, icon: Icon, tip }) => (
          <div key={label} className="card" style={{ padding: 16, display: "flex", gap: 14, alignItems: "center" }} title={tip}>
            <IconTile size={38} radius={10} bg="var(--color-accent-100)" fg="var(--color-accent-700)">
              <Icon size={17} />
            </IconTile>
            <div>
              <div style={{ fontSize: 26, fontWeight: 700, lineHeight: 1.1 }}>{value}</div>
              <div style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>{label}</div>
            </div>
          </div>
        ))}
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 22, marginBottom: 30 }}>
        <section>
          <SectionTitle>My assists</SectionTitle>
          <div className="card" style={{ padding: "6px 14px" }}>
            {record.my_assists.length === 0 && (
              <p style={{ fontSize: 13, color: "var(--color-neutral-600)" }}>Nothing yet.</p>
            )}
            {record.my_assists.map((row) => (
              <button
                key={`${row.ref}-${row.role}`}
                onClick={() => navigate({ name: "assist", ref: row.ref })}
                style={{
                  display: "grid",
                  gridTemplateColumns: "18px 58px 1fr 76px 54px",
                  alignItems: "center",
                  gap: 8,
                  width: "100%",
                  border: "none",
                  borderBottom: "1px solid var(--color-neutral-200)",
                  background: "transparent",
                  padding: "9px 0",
                  cursor: "pointer",
                  font: "inherit",
                  fontSize: 12.5,
                  textAlign: "left",
                  color: "inherit",
                }}
              >
                <StatusDot status={row.status} size={8} />
                <span style={{ fontWeight: 700, color: "var(--color-neutral-600)", fontSize: 11.5 }}>
                  {row.ref}
                </span>
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                  title={row.title}
                >
                  {row.title}
                </span>
                <span style={{ fontSize: 11, color: "var(--color-neutral-500)" }}>
                  {row.role === "owner"
                    ? row.outcome
                      ? OUTCOME_LABELS[row.outcome]
                      : "owner"
                    : "responder"}
                </span>
                <span style={{ display: "flex" }}>
                  {row.responder_names.slice(0, 3).map((n) => (
                    <span key={n} style={{ marginLeft: -6 }}>
                      <AvatarChip name={n} size={20} />
                    </span>
                  ))}
                </span>
              </button>
            ))}
          </div>
        </section>

        <section>
          <SectionTitle>Credits earned</SectionTitle>
          <div className="card" style={{ padding: "6px 14px" }}>
            {record.credits_rows.length === 0 && (
              <p style={{ fontSize: 13, color: "var(--color-neutral-600)" }}>
                None yet. Respond to an assist; credit is the owner's to give.
              </p>
            )}
            {record.credits_rows.map((row) => (
              <div
                key={row.assist_ref}
                style={{
                  display: "grid",
                  gridTemplateColumns: "18px 58px 1fr 54px",
                  alignItems: "center",
                  gap: 8,
                  borderBottom: "1px solid var(--color-neutral-200)",
                  padding: "9px 0",
                  fontSize: 12.5,
                }}
              >
                <Award size={14} color="var(--color-accent-700)" />
                <span style={{ fontWeight: 700, color: "var(--color-neutral-600)", fontSize: 11.5 }}>
                  {row.assist_ref}
                </span>
                <span
                  style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                  title={row.title}
                >
                  {row.title}
                </span>
                <span title={`credited by ${row.from_owner_name}`} style={{ justifySelf: "end" }}>
                  <AvatarChip name={row.from_owner_name} size={22} />
                </span>
              </div>
            ))}
          </div>
        </section>
      </div>

      <section>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 12 }}>
          <Bot size={16} color="var(--color-neutral-700)" />
          <SectionTitle>AI usage</SectionTitle>
          <span style={{ fontSize: 11.5, color: "var(--color-neutral-500)" }}>
            measured locally by the detector - visible only to you
          </span>
          <div style={{ flex: 1 }} />
          {record.ai_usage.map((u) => (
            <button
              key={u.range}
              className={`btn${u.range === range ? " btn-on" : ""}`}
              style={{ padding: "4px 10px", fontSize: 12 }}
              onClick={() => setRange(u.range)}
            >
              {u.range}
            </button>
          ))}
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14, marginBottom: 14 }}>
          {[
            { label: "Tokens", value: usage.tokens },
            { label: "Spend", value: usage.spend },
            { label: "Longest stall", value: usage.longest_stall },
          ].map(({ label, value }) => (
            <div key={label} className="card" style={{ padding: 14 }}>
              <div style={{ fontSize: 20, fontWeight: 700 }}>{value}</div>
              <div style={{ fontSize: 12, color: "var(--color-neutral-600)" }}>{label}</div>
            </div>
          ))}
        </div>

        <div className="card" style={{ padding: "6px 14px" }}>
          {usage.agents.map((agent) => (
            <div
              key={agent.name}
              style={{
                display: "grid",
                gridTemplateColumns: "130px 100px minmax(80px, 1fr) 64px 74px",
                alignItems: "center",
                gap: 10,
                borderBottom: "1px solid var(--color-neutral-200)",
                padding: "9px 0",
                fontSize: 12.5,
              }}
            >
              <span style={{ fontWeight: 600 }}>{agent.name}</span>
              <span style={{ color: "var(--color-neutral-600)" }}>{agent.model}</span>
              <span style={{ background: "var(--color-neutral-200)", borderRadius: 4, height: 8 }}>
                <span
                  style={{
                    display: "block",
                    width: `${agent.share_pct}%`,
                    height: "100%",
                    borderRadius: 4,
                    background: "var(--color-accent)",
                  }}
                />
              </span>
              <span className="mono" style={{ fontSize: 11.5 }}>{agent.tokens}</span>
              <span className="mono" style={{ fontSize: 11.5 }}>{agent.spend}</span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
