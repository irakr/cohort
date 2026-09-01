// Forwards the webview's console output into the app log file
// (<OS config dir>/cohort/app.log) via the Tauri log plugin, so frontend
// and Rust logs land in one place. No-op outside Tauri (tests, browser dev).

import { inTauri } from "./api/tauri";

type Forwarder = (message: string) => Promise<void>;

function format(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === "string") {
        return a;
      }
      if (a instanceof Error) {
        return a.stack ?? a.message;
      }
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

function forward(name: "log" | "info" | "warn" | "error" | "debug", logger: Forwarder) {
  const original = console[name].bind(console);
  console[name] = (...args: unknown[]) => {
    original(...args);
    void logger(format(args)).catch(() => {
      // never let logging break the app
    });
  };
}

export async function initLogging(): Promise<void> {
  if (!inTauri()) {
    return;
  }
  try {
    const { debug, error, info, warn } = await import("@tauri-apps/plugin-log");
    forward("log", info);
    forward("info", info);
    forward("warn", warn);
    forward("error", error);
    forward("debug", debug);
    void info("webview console attached to app.log");
  } catch {
    // plugin unavailable; console keeps working locally
  }
}
