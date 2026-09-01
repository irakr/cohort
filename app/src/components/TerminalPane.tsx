import { useEffect, useRef, useState } from "react";

/** Read-only terminal activity view, fed by the owner's engine. The feed is
    one list with `== <terminal>` section headers; each tab shows only its
    own section. Header (`==`) and command (`$`) lines are accented. */
export function TerminalPane({ tabs, feed }: { tabs: string[]; feed: string[] }) {
  const [activeTab, setActiveTab] = useState(0);
  const bodyRef = useRef<HTMLDivElement>(null);

  // Split the merged feed into per-terminal sections. A feed without
  // section headers (e.g. seeded assists) is shown whole under every tab.
  const sections: { header: string; lines: string[] }[] = [];
  for (const line of feed) {
    if (line.startsWith("== ")) {
      sections.push({ header: line, lines: [line] });
    } else if (sections.length > 0) {
      sections[sections.length - 1].lines.push(line);
    }
  }
  const activeLabel = tabs[Math.min(activeTab, tabs.length - 1)] ?? "";
  const lines =
    sections.length === 0
      ? feed
      : sections.find((s) => s.header.includes(activeLabel))?.lines ?? [
          `== ${activeLabel}`,
          "  (no activity captured yet)",
        ];

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [feed, activeTab]);

  return (
    <div>
      <div style={{ display: "flex", gap: 4 }}>
        {tabs.map((tab, i) => (
          <button
            key={tab}
            onClick={() => setActiveTab(i)}
            style={{
              border: "none",
              cursor: "pointer",
              font: "inherit",
              fontSize: 12,
              fontWeight: 600,
              padding: "6px 12px",
              borderRadius: "8px 8px 0 0",
              background: i === activeTab ? "var(--color-neutral-900)" : "var(--color-neutral-200)",
              color: i === activeTab ? "#fff" : "var(--color-neutral-700)",
            }}
          >
            {tab}
          </button>
        ))}
      </div>
      <div
        ref={bodyRef}
        className="mono"
        style={{
          background: "var(--color-neutral-900)",
          color: "var(--color-neutral-200)",
          borderRadius: "0 8px 8px 8px",
          padding: "12px 14px",
          fontSize: 12,
          lineHeight: 1.6,
          minHeight: 250,
          maxHeight: 300,
          overflowY: "auto",
          whiteSpace: "pre-wrap",
        }}
      >
        {lines.map((line, i) => (
          <div
            key={i}
            style={{
              color:
                line.startsWith("$") || line.startsWith("==")
                  ? "var(--color-accent-300)"
                  : undefined,
            }}
          >
            {line === "" ? "\u00a0" : line}
          </div>
        ))}
      </div>
    </div>
  );
}
