import { vi } from "vitest";
import type { AssistSummary, User } from "../api/types";

// Fixture data, not product data: the hub ships empty. Names are roles so
// each test reads as who-does-what, and refs follow the real 'S-<n>' scheme.
export const USERS: User[] = [
  { id: "u-owner", name: "Owner", initials: "O" },
  { id: "u-responder", name: "Responder", initials: "R" },
  { id: "u-bystander", name: "Bystander", initials: "B" },
];

export const ASSISTS: AssistSummary[] = [
  {
    ref: "S-4",
    title: "Rollout hangs on an image pull",
    status: "open",
    category: "broken",
    tags: ["kubernetes", "helm", "registry-auth"],
    owner_name: "Bystander",
    responder_names: [],
    created_at: new Date(Date.now() - 22 * 60000).toISOString(),
    is_mine: false,
  },
  {
    ref: "S-3",
    title: "Migration deadlocks only under the test harness",
    status: "open",
    category: "broken",
    tags: ["postgres", "rust", "sqlx"],
    owner_name: "Owner",
    responder_names: ["Responder", "Bystander"],
    created_at: new Date(Date.now() - 60 * 60000).toISOString(),
    is_mine: true,
  },
  {
    // Nothing in the hub sets 'dormant' yet; the fixture carries one so the
    // status filter itself stays covered.
    ref: "S-2",
    title: "Vite build runs out of memory in CI",
    status: "dormant",
    category: "environment",
    tags: ["ci", "node", "build"],
    owner_name: "Bystander",
    responder_names: [],
    created_at: new Date(Date.now() - 180 * 60000).toISOString(),
    is_mine: false,
  },
  {
    ref: "S-1",
    title: "gRPC stream closes at exactly 60s",
    status: "done",
    category: "broken",
    tags: ["networking", "envoy", "grpc"],
    owner_name: "Responder",
    responder_names: ["Owner"],
    created_at: new Date(Date.now() - 300 * 60000).toISOString(),
    is_mine: true,
  },
];

/** Install a fetch mock that answers hub endpoints from the fixtures,
    honoring the list filters the same way the hub does. */
export function mockHubFetch() {
  // Registering adds to the list the hub serves, as the real one does: the
  // app verifies its stored identity against /api/users on boot.
  const users = [...USERS];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input));
    let payload: unknown = { error: `no fixture for ${url.pathname}` };
    if (url.pathname === "/api/assists/S-3" && init?.method === "DELETE") {
      payload = { status: "deleted", ref: "S-3" };
    } else if (url.pathname === "/api/assists/S-3") {
      payload = {
        ref: "S-3",
        title: "Migration deadlocks only under the test harness",
        status: "open",
        category: "broken",
        tags: ["postgres"],
        owner_id: "u-owner",
        owner_name: "Owner",
        anonymous: false,
        description: "Deadlocks under the harness.",
        insights: "",
        environment: ["Postgres 16"],
        artifacts: [],
        responders: [{ id: "u-responder", name: "Responder", initials: "R" }],
        scope_requests: [],
        grants: [],
        catalog: [],
        catalog_at: null,
        viewer_is_owner: true,
        viewer_is_responder: false,
        created_at: new Date(Date.now() - 60 * 60000).toISOString(),
        closed_at: null,
      };
    } else if (url.pathname === "/api/assists/S-3/artifacts") {
      payload = { file_tree: [], files: {}, agent_chat: [] };
    } else if (url.pathname === "/api/users" && init?.method === "POST") {
      const body = JSON.parse(String(init.body)) as { name: string };
      const user = {
        id: `u-${body.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`,
        name: body.name.trim(),
        initials: body.name.trim().charAt(0).toUpperCase(),
      };
      users.push(user);
      payload = user;
    } else if (url.pathname === "/api/users") {
      payload = users;
    } else if (url.pathname === "/api/notifications") {
      payload = { now: new Date().toISOString(), notifications: [] };
    } else if (url.pathname === "/api/assists" && init?.method === "POST") {
      const body = JSON.parse(String(init.body)) as Record<string, unknown>;
      payload = {
        ref: "S-9",
        title: body.title,
        status: "open",
        category: body.category ?? null,
        tags: body.tags ?? [],
        owner_id: "u-owner",
        owner_name: "Owner",
        anonymous: false,
        description: body.description ?? "",
        insights: body.insights ?? "",
        environment: body.environment ?? [],
        artifacts: body.artifacts ?? [],
        responders: [],
        scope_requests: [],
        grants: [],
        catalog: [],
        catalog_at: null,
        viewer_is_owner: true,
        viewer_is_responder: false,
        created_at: new Date().toISOString(),
        closed_at: null,
      };
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
