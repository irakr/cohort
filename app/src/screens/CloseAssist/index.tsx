import { useEffect, useState } from "react";
import { Award } from "lucide-react";
import { apiPost } from "../../api/client";
import { useApi } from "../../api/hooks";
import type { AssistDetail, Outcome, RecordFields } from "../../api/types";
import { AvatarChip, SectionTitle, Spinner } from "../../components/ui";
import { OUTCOME_LABELS } from "../../util";
import { useNav } from "../../app/router";

const RECORD_ROWS: { key: keyof RecordFields; label: string; height: number }[] = [
  { key: "symptom", label: "Symptom", height: 52 },
  { key: "env_fingerprint", label: "Env fingerprint", height: 52 },
  { key: "scopes_that_mattered", label: "Scopes that mattered", height: 52 },
  { key: "dead_ends", label: "Dead ends", height: 52 },
  { key: "fix", label: "Fix", height: 72 },
];

export function CloseAssist({ assistRef }: { assistRef: string }) {
  const { navigate } = useNav();
  const { data: assist } = useApi<AssistDetail>(`/api/assists/${assistRef}`);
  const { data: draft } = useApi<RecordFields>(`/api/assists/${assistRef}/record-draft`);
  const [outcome, setOutcome] = useState<Outcome>("resolved");
  const [credited, setCredited] = useState<Record<string, boolean>>({});
  const [record, setRecord] = useState<RecordFields | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (draft && record === null) {
      setRecord(draft);
    }
  }, [draft, record]);

  if (!assist || !record) {
    return (
      <div style={{ display: "grid", placeItems: "center", minHeight: "60vh" }}>
        <Spinner size={22} />
      </div>
    );
  }

  async function save() {
    setBusy(true);
    setError(null);
    try {
      await apiPost(`/api/assists/${assistRef}/close`, {
        outcome,
        credited_user_ids: Object.keys(credited).filter((id) => credited[id]),
        record,
      });
      navigate({ name: "assists" });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  return (
    <div style={{ maxWidth: 760, margin: "0 auto", padding: "34px 28px" }}>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 4 }}>Close assist</h1>
      <p style={{ color: "var(--color-neutral-600)", fontSize: 13.5, margin: "0 0 22px" }}>
        A resolution record is written on every close. It carries no code, no screenshots,
        no transcript.
      </p>

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div className="card" style={{ padding: 18 }}>
          <SectionTitle>Outcome</SectionTitle>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {(Object.keys(OUTCOME_LABELS) as Outcome[]).map((o) => (
              <button
                key={o}
                className={`btn${outcome === o ? " btn-on" : ""}`}
                onClick={() => setOutcome(o)}
              >
                {OUTCOME_LABELS[o]}
              </button>
            ))}
          </div>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <SectionTitle>Credit who helped</SectionTitle>
          {assist.responders.length === 0 ? (
            <p style={{ fontSize: 13, color: "var(--color-neutral-600)", margin: 0 }}>
              No responders joined this assist.
            </p>
          ) : (
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {assist.responders.map((r) => {
                const on = !!credited[r.id];
                return (
                  <button
                    key={r.id}
                    onClick={() => setCredited((c) => ({ ...c, [r.id]: !c[r.id] }))}
                    style={{
                      display: "inline-flex",
                      alignItems: "center",
                      gap: 8,
                      border: `1px solid ${on ? "var(--color-accent-300)" : "var(--color-neutral-300)"}`,
                      background: on ? "var(--color-accent-100)" : "#fff",
                      borderRadius: 999,
                      padding: "5px 12px 5px 5px",
                      cursor: "pointer",
                      font: "inherit",
                      fontSize: 13,
                      fontWeight: 600,
                    }}
                  >
                    <AvatarChip name={r.name} active={on} />
                    {r.name}
                    {on && <Award size={14} color="var(--color-accent-700)" />}
                  </button>
                );
              })}
            </div>
          )}
          <p style={{ fontSize: 11.5, color: "var(--color-neutral-500)", margin: "10px 0 0" }}>
            optional - never prompted twice
          </p>
        </div>

        <div className="card" style={{ padding: 18 }}>
          <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
            <SectionTitle>Resolution record (auto-drafted, editable)</SectionTitle>
            <span style={{ fontSize: 11, color: "var(--color-neutral-500)" }}>
              class B - kept indefinitely
            </span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {RECORD_ROWS.map(({ key, label, height }) => (
              <div key={key} style={{ display: "grid", gridTemplateColumns: "150px 1fr", gap: 10 }}>
                <label style={{ fontSize: 12.5, fontWeight: 600, color: "var(--color-neutral-700)", paddingTop: 8 }}>
                  {label}
                </label>
                <textarea
                  className="input mono"
                  style={{ minHeight: height, fontSize: 12.5 }}
                  value={record[key]}
                  onChange={(e) => setRecord((r) => ({ ...(r as RecordFields), [key]: e.target.value }))}
                />
              </div>
            ))}
          </div>
        </div>

        {error && <div style={{ color: "var(--color-accent-700)", fontSize: 13 }}>{error}</div>}
        <div style={{ display: "flex", gap: 10 }}>
          <button className="btn btn-primary" disabled={busy} onClick={() => void save()}>
            Save record and close
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => navigate({ name: "assist", ref: assistRef })}
          >
            Back to assist
          </button>
        </div>
      </div>
    </div>
  );
}
