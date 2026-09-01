/** "22 min" / "1 h" / "3 d" style age from an RFC3339 timestamp. */
export function timeAgo(iso: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) {
    return "";
  }
  const minutes = Math.max(0, Math.round((Date.now() - then) / 60000));
  if (minutes < 60) {
    return `${minutes} min`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours} h`;
  }
  return `${Math.round(hours / 24)} d`;
}

/** "in 4 h" / "in 12 min" / "expired" for a future RFC3339 timestamp. */
export function timeUntil(iso: string): string {
  const then = Date.parse(iso);
  if (Number.isNaN(then)) {
    return "";
  }
  const minutes = Math.round((then - Date.now()) / 60000);
  if (minutes <= 0) {
    return "expired";
  }
  if (minutes < 60) {
    return `in ${minutes} min`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `in ${hours} h`;
  }
  return `in ${Math.round(hours / 24)} d`;
}

/** Minimal markdown for the brief goal: bold, italics, code, links, bullets. */
export function renderMarkdown(src: string): string {
  const escape = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const inline = (s: string) =>
    s
      .replace(
        /`([^`]+)`/g,
        '<code style="font-family:var(--font-mono);font-size:0.9em;background:var(--color-neutral-200);border-radius:4px;padding:1px 5px">$1</code>',
      )
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/\*([^*]+)\*/g, "<em>$1</em>")
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
  const lines = escape(src).split("\n");
  let html = "";
  let inList = false;
  for (const line of lines) {
    if (/^\s*[-*] /.test(line)) {
      if (!inList) {
        html += '<ul style="margin:4px 0;padding-left:20px">';
        inList = true;
      }
      html += `<li>${inline(line.replace(/^\s*[-*] /, ""))}</li>`;
    } else {
      if (inList) {
        html += "</ul>";
        inList = false;
      }
      if (line.trim()) {
        html += `<p style="margin:0 0 4px">${inline(line)}</p>`;
      }
    }
  }
  if (inList) {
    html += "</ul>";
  }
  return html;
}

export const OUTCOME_LABELS: Record<string, string> = {
  resolved: "Resolved",
  worked_around: "Worked around",
  abandoned: "Abandoned",
  self_resolved: "Self-resolved",
};

export const CATEGORY_LABELS: Record<string, string> = {
  broken: "Broken",
  environment: "Environment",
  approach: "Approach",
  review: "Review",
  knowledge: "Knowledge",
  agent_loop: "Agent loop",
};
