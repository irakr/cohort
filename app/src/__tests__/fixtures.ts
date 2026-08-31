import { vi } from "vitest";
import type { AssistSummary, User } from "../api/types";

export const USERS: User[] = [
  { id: "u-alex", name: "Alex", initials: "A" },
  { id: "u-meera", name: "Meera N.", initials: "M" },
  { id: "u-priya", name: "Priya", initials: "P" },
];

export const ASSISTS: AssistSummary[] = [
  {
    ref: "S-2411",
    title: "Need help with an image pull that keeps failing on staging",
    status: "open",
    category: "broken",
    tags: ["kubernetes", "helm", "registry-auth"],
    owner_name: "Meera N.",
    responder_names: [],
    created_at: new Date(Date.now() - 22 * 60000).toISOString(),
    is_mine: false,
  },
  {
    ref: "S-2409",
    title: "My migration deadlocks only under the test harness and I'm out of ideas",
    status: "open",
    category: "broken",
    tags: ["postgres", "rust", "sqlx"],
    owner_name: "Alex",
    responder_names: ["Priya", "Arun"],
    created_at: new Date(Date.now() - 60 * 60000).toISOString(),
    is_mine: true,
  },
  {
    ref: "S-2404",
    title: "Can't figure out why our Vite build OOMs in CI but passes locally",
    status: "dormant",
    category: "environment",
    tags: ["ci", "node", "build"],
    owner_name: "Devansh R.",
    responder_names: [],
    created_at: new Date(Date.now() - 180 * 60000).toISOString(),
    is_mine: false,
  },
  {
    ref: "S-2398",
    title: "Our gRPC stream was closing at exactly 60s behind the proxy",
    status: "done",
    category: "broken",
    tags: ["networking", "envoy", "grpc"],
    owner_name: "Anika S.",
    responder_names: ["Alex"],
    created_at: new Date(Date.now() - 300 * 60000).toISOString(),
    is_mine: true,
  },
];

/** Install a fetch mock that answers hub endpoints from the fixtures,
    honoring the list filters the same way the hub does. */
export function mockHubFetch() {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input));
    let payload: unknown = { error: `no fixture for ${url.pathname}` };
    if (url.pathname === "/api/users" && init?.method === "POST") {
      const body = JSON.parse(String(init.body)) as { name: string };
      payload = {
        id: `u-${body.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`,
        name: body.name.trim(),
        initials: body.name.trim().charAt(0).toUpperCase(),
      };
    } else if (url.pathname === "/api/users") {
      payload = USERS;
    } else if (url.pathname === "/api/notifications") {
      payload = { now: new Date().toISOString(), notifications: [] };
    } else if (url.pathname === "/api/assists") {
      let rows = ASSISTS;
      const status = url.searchParams.get("status");
      if (status) {
        const set = status.split(",");
        rows = rows.filter((a) => set.includes(a.status));
      }
      const tag = url.searchParams.get("tag");
      if (tag) {
        rows = rows.filter((a) => a.tags.includes(tag));
      }
      if (url.searchParams.get("mine") === "true") {
        rows = rows.filter((a) => a.is_mine);
      }
      payload = rows;
    }
    return new Response(JSON.stringify(payload), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}
