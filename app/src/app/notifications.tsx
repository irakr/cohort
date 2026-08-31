import { useCallback, useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { apiGet } from "../api/client";
import type { HubNotification, NotificationsResponse } from "../api/types";
import { useNav } from "./router";

const POLL_MS = 5000;
const TOAST_MS = 9000;
const MAX_SEEN = 500;

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function notifyNative(n: HubNotification) {
  if (!inTauri()) {
    return;
  }
  try {
    const { isPermissionGranted, requestPermission, sendNotification } = await import(
      "@tauri-apps/plugin-notification"
    );
    let granted = await isPermissionGranted();
    if (!granted) {
      granted = (await requestPermission()) === "granted";
    }
    if (granted) {
      sendNotification({ title: `${n.assist_ref}: ${n.assist_title}`, body: n.message });
    }
  } catch {
    // plugin unavailable; the in-app toast still shows
  }
}

/** Polls the hub for events for this identity (requests on assists you own,
    decisions on your requests, joins, credits) and surfaces them as in-app
    toasts plus native desktop notifications. The cursor is inclusive
    server-side, so events are deduped by id here. */
export function Notifications() {
  const { currentUserId, navigate } = useNav();
  const [toasts, setToasts] = useState<HubNotification[]>([]);
  const cursor = useRef<string | null>(null);
  const seen = useRef<Set<string>>(new Set());

  const dismiss = useCallback((id: string) => {
    setToasts((list) => list.filter((t) => t.id !== id));
  }, []);

  useEffect(() => {
    if (!currentUserId) {
      return;
    }
    cursor.current = null;
    seen.current = new Set();
    let stopped = false;

    const poll = async () => {
      try {
        const path = cursor.current
          ? `/api/notifications?since=${encodeURIComponent(cursor.current)}`
          : "/api/notifications";
        const response = await apiGet<NotificationsResponse>(path);
        if (stopped) {
          return;
        }
        cursor.current = response.now;
        const fresh = response.notifications.filter((n) => !seen.current.has(n.id));
        for (const n of fresh) {
          seen.current.add(n.id);
          void notifyNative(n);
        }
        if (seen.current.size > MAX_SEEN) {
          seen.current = new Set([...seen.current].slice(-MAX_SEEN / 2));
        }
        if (fresh.length > 0) {
          setToasts((list) => [...list, ...fresh].slice(-4));
          for (const n of fresh) {
            setTimeout(() => dismiss(n.id), TOAST_MS);
          }
        }
      } catch {
        // hub unreachable; retry on the next tick
      }
    };

    void poll();
    const timer = setInterval(() => void poll(), POLL_MS);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [currentUserId, dismiss]);

  if (toasts.length === 0) {
    return null;
  }
  return (
    <div
      style={{
        position: "fixed",
        right: 18,
        bottom: 18,
        display: "flex",
        flexDirection: "column",
        gap: 8,
        zIndex: 80,
        width: 340,
      }}
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          className="card fade-in"
          style={{ display: "flex", gap: 10, padding: "12px 14px", boxShadow: "var(--shadow-lg)" }}
        >
          <button
            onClick={() => {
              dismiss(t.id);
              navigate({ name: "assist", ref: t.assist_ref });
            }}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              font: "inherit",
              textAlign: "left",
              padding: 0,
              flex: 1,
              minWidth: 0,
              color: "inherit",
            }}
          >
            <div style={{ fontSize: 11.5, fontWeight: 700, color: "var(--color-accent-700)", marginBottom: 2 }}>
              {t.assist_ref} - {t.assist_title}
            </div>
            <div style={{ fontSize: 13 }}>{t.message}</div>
          </button>
          <button
            onClick={() => dismiss(t.id)}
            title="Dismiss"
            style={{ border: "none", background: "transparent", cursor: "pointer", color: "var(--color-neutral-500)", padding: 0, alignSelf: "flex-start" }}
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
