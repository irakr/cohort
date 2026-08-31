import { useEffect, useRef, useState } from "react";

/** Read-only terminal stream, replayed from the assist's seeded feed. */
export function TerminalPane({ tabs, feed }: { tabs: string[]; feed: string[] }) {
  const [activeTab, setActiveTab] = useState(0);
  const [visibleCount, setVisibleCount] = useState(Math.min(3, feed.length));
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (visibleCount >= feed.length) {
      return;
    }
    const timer = setInterval(
      () => setVisibleCount((n) => Math.min(n + 1, feed.length)),
      2600,
    );
    return () => clearInterval(timer);
  }, [visibleCount, feed.length]);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [visibleCount]);

  const lines = feed.slice(0, visibleCount).slice(-14);

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
            style={{ color: line.startsWith("$") ? "var(--color-accent-300)" : undefined }}
          >
            {line}
          </div>
        ))}
      </div>
    </div>
  );
}
