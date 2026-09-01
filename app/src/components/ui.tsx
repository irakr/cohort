import type { CSSProperties, ReactNode } from "react";
import { Loader2 } from "lucide-react";
import type { AssistStatus } from "../api/types";

export const STATUS_META: Record<AssistStatus, { color: string; label: string; tip: string }> = {
  open: { color: "var(--color-accent)", label: "open", tip: "Open, needs a responder" },
  dormant: { color: "var(--color-warning)", label: "dormant", tip: "Dormant, quiet for hours, still open" },
  done: { color: "var(--color-success)", label: "completed", tip: "Completed, resolution record kept" },
};

export function StatusDot({ status, size = 9 }: { status: AssistStatus; size?: number }) {
  return (
    <span
      title={STATUS_META[status].tip}
      style={{
        display: "inline-block",
        width: size,
        height: size,
        borderRadius: "50%",
        background: STATUS_META[status].color,
        flexShrink: 0,
      }}
    />
  );
}

const PILL_STYLES: Record<AssistStatus, { bg: string; fg: string }> = {
  open: { bg: "var(--color-accent-100)", fg: "var(--color-accent-700)" },
  dormant: { bg: "var(--color-warning-bg)", fg: "var(--color-warning-fg)" },
  done: { bg: "var(--color-success-bg)", fg: "var(--color-success-fg)" },
};

export function StatusPill({ status }: { status: AssistStatus }) {
  const style = PILL_STYLES[status];
  return (
    <span
      className="tag"
      style={{
        background: style.bg,
        color: style.fg,
        textTransform: "uppercase",
        letterSpacing: "0.06em",
        fontSize: 10.5,
      }}
    >
      {STATUS_META[status].label}
    </span>
  );
}

export function AvatarChip({
  name,
  active = false,
  size = 28,
}: {
  name: string;
  active?: boolean;
  size?: number;
}) {
  return (
    <span
      title={name}
      style={{
        display: "inline-grid",
        placeItems: "center",
        width: size,
        height: size,
        borderRadius: "50%",
        fontSize: size * 0.45,
        fontWeight: 700,
        background: active ? "var(--color-accent)" : "var(--color-neutral-200)",
        color: active ? "#fff" : "var(--color-neutral-700)",
        flexShrink: 0,
      }}
    >
      {name.charAt(0)}
    </span>
  );
}

export function Spinner({ size = 16 }: { size?: number }) {
  return <Loader2 className="spin" size={size} strokeWidth={2.2} aria-label="loading" />;
}

export function Modal({
  width,
  onClose,
  children,
}: {
  width: number;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="modal-panel fade-in"
        style={{ width, maxWidth: "92vw" }}
        onClick={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}

/** In-app confirmation dialog (the webview has no native confirm()). */
export function ConfirmDialog({
  title,
  message,
  confirmLabel,
  busy = false,
  onCancel,
  onConfirm,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal width={420} onClose={onCancel}>
      <h3 style={{ fontSize: 16, marginBottom: 8 }}>{title}</h3>
      <p style={{ fontSize: 13.5, color: "var(--color-neutral-700)", margin: "0 0 16px", lineHeight: 1.5 }}>
        {message}
      </p>
      <div style={{ display: "flex", gap: 10, justifyContent: "flex-end" }}>
        <button className="btn" onClick={onCancel}>
          Cancel
        </button>
        <button className="btn btn-primary" disabled={busy} onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

/** In-app notice dialog, for errors and one-off messages. */
export function NoticeDialog({ message, onClose }: { message: string; onClose: () => void }) {
  return (
    <Modal width={420} onClose={onClose}>
      <p style={{ fontSize: 13.5, color: "var(--color-neutral-700)", margin: "0 0 16px", lineHeight: 1.5 }}>
        {message}
      </p>
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button className="btn btn-primary" onClick={onClose}>
          OK
        </button>
      </div>
    </Modal>
  );
}

export function IconTile({
  bg,
  fg = "#fff",
  size = 30,
  radius = 8,
  fontSize = 10,
  children,
  style,
}: {
  bg: string;
  fg?: string;
  size?: number;
  radius?: number;
  fontSize?: number;
  children: ReactNode;
  style?: CSSProperties;
}) {
  return (
    <span
      style={{
        display: "inline-grid",
        placeItems: "center",
        width: size,
        height: size,
        borderRadius: radius,
        background: bg,
        color: fg,
        fontSize,
        fontWeight: 700,
        flexShrink: 0,
        ...style,
      }}
    >
      {children}
    </span>
  );
}

export function SectionTitle({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        fontSize: 11,
        fontWeight: 700,
        textTransform: "uppercase",
        letterSpacing: "0.08em",
        color: "var(--color-neutral-600)",
        margin: "0 0 10px",
      }}
    >
      {children}
    </div>
  );
}
