import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import App from "../App";

/** A hub with no users at all: what a freshly installed hub looks like. */
function mockFreshHub() {
  const registered: { id: string; name: string; initials: string }[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input));
    const identity = (init?.headers as Record<string, string> | undefined)?.["X-User-Id"];
    const json = (payload: unknown, status = 200) =>
      new Response(JSON.stringify(payload), {
        status,
        headers: { "Content-Type": "application/json" },
      });

    if (url.pathname === "/api/users" && init?.method === "POST") {
      const { name } = JSON.parse(String(init.body)) as { name: string };
      const user = {
        id: `u-${name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`,
        name: name.trim(),
        initials: name.trim().charAt(0).toUpperCase(),
      };
      registered.push(user);
      return json(user);
    }
    if (url.pathname === "/api/users") {
      return json(registered);
    }
    // Everything else needs an identity the hub knows, exactly like the hub.
    if (!identity) {
      return json({ error: "the X-User-Id header is required" }, 403);
    }
    if (!registered.some((u) => u.id === identity)) {
      return json({ error: `unknown user '${identity}'` }, 403);
    }
    if (url.pathname === "/api/notifications") {
      return json({ now: new Date().toISOString(), notifications: [] });
    }
    return json([]);
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("First run on a fresh install", () => {
  it("a machine with no identity must create one before seeing anything", async () => {
    localStorage.clear();
    const fetchMock = mockFreshHub();
    render(<App />);

    // Setup, not the app: there is nothing to see without an identity.
    await waitFor(() => expect(screen.getByText(/Connect to your team's hub/)).toBeTruthy());
    expect(screen.queryByText("Assists")).toBeNull();

    // A hub with no users offers registration and no one to continue as.
    expect(screen.getByPlaceholderText("Your name")).toBeTruthy();
    expect(screen.queryByText(/continue as an existing user/)).toBeNull();

    await userEvent.type(screen.getByPlaceholderText("Your name"), "Ira K.");
    await userEvent.click(screen.getByRole("button", { name: "Join" }));

    // Registered, persisted to this machine, and now inside the app.
    await waitFor(() => expect(localStorage.getItem("cohort.userId")).toBe("u-ira-k"));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    expect(
      fetchMock.mock.calls.some(
        (c) => String(c[0]).endsWith("/api/users") && c[1]?.method === "POST",
      ),
    ).toBe(true);
  });

  it("a second machine signs in as an existing user instead of registering", async () => {
    localStorage.clear();
    const fetchMock = mockFreshHub();
    // Someone already registered on this hub.
    await fetch("http://127.0.0.1:7400/api/users", {
      method: "POST",
      body: JSON.stringify({ name: "Ira K." }),
    });

    render(<App />);
    await waitFor(() => expect(screen.getByRole("button", { name: /Ira K\./ })).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: /Ira K\./ }));

    await waitFor(() => expect(localStorage.getItem("cohort.userId")).toBe("u-ira-k"));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    const registerCalls = fetchMock.mock.calls.filter(
      (c) => String(c[0]).endsWith("/api/users") && c[1]?.method === "POST",
    );
    expect(registerCalls.length).toBe(1); // only the setup call above
  });

  it("an identity the hub no longer knows returns the machine to setup", async () => {
    // The exact state of a machine whose hub database was reset.
    localStorage.setItem("cohort.userId", "u-from-the-old-database");
    const fetchMock = mockFreshHub();
    render(<App />);

    await waitFor(() => expect(screen.getByText(/Connect to your team's hub/)).toBeTruthy());
    expect(localStorage.getItem("cohort.userId")).toBeNull();

    // The app checks before it mounts anything, so the only request made as
    // that identity is the check itself - a route that needs no identity and
    // answers 200. No burst of 403s reaches the console or app.log.
    const asStaleUser = fetchMock.mock.calls
      .filter((c) => (c[1]?.headers as Record<string, string> | undefined)?.["X-User-Id"])
      .map((c) => new URL(String(c[0])).pathname);
    expect(asStaleUser).toEqual(["/api/users"]);

    // And registering again works from there, once setup has reached the hub.
    await waitFor(() => expect(screen.getByPlaceholderText("Your name")).toBeTruthy());
    await userEvent.type(screen.getByPlaceholderText("Your name"), "Ira K.");
    await userEvent.click(screen.getByRole("button", { name: "Join" }));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
  });

  it("an unreachable hub is not a sign-out", async () => {
    localStorage.setItem("cohort.userId", "u-ira-k");
    vi.stubGlobal("fetch", vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    }));
    render(<App />);

    // The identity survives a network failure: it was never rejected.
    await new Promise((r) => setTimeout(r, 50));
    expect(localStorage.getItem("cohort.userId")).toBe("u-ira-k");
    expect(screen.queryByText(/Connect to your team's hub/)).toBeNull();
  });
});
