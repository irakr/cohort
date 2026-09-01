import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { NavProvider } from "../app/router";
import { AssistDetail } from "../screens/AssistDetail";
import { mockHubFetch } from "./fixtures";

vi.mock("../api/agent", () => ({
  suggestArtifacts: vi.fn(async () => []),
  envFingerprint: vi.fn(async () => []),
  snapshotPaths: vi.fn(async () => null),
  terminalActivity: vi.fn(async () => null),
  sshPublicKey: vi.fn(async () => null),
  sshTargetSuggestion: vi.fn(async () => ""),
  installSshKey: vi.fn(async () => false),
  openSsh: vi.fn(async () => false),
}));

describe("Delete assist", () => {
  it("confirms in an in-app dialog, then issues the DELETE", async () => {
    const fetchMock = mockHubFetch();
    render(
      <NavProvider>
        <AssistDetail assistRef="S-2409" />
      </NavProvider>,
    );

    await waitFor(() => expect(screen.getByText(/migration deadlocks/)).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: /Delete assist/ }));

    // In-app confirmation, not a native dialog.
    expect(screen.getByText(/Delete S-2409 and everything shared on it/)).toBeTruthy();
    expect(fetchMock.mock.calls.every((c) => c[1]?.method !== "DELETE")).toBe(true);

    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          (c) => c[1]?.method === "DELETE" && String(c[0]).endsWith("/api/assists/S-2409"),
        ),
      ).toBe(true);
    });
  });

  it("cancel keeps the assist", async () => {
    const fetchMock = mockHubFetch();
    render(
      <NavProvider>
        <AssistDetail assistRef="S-2409" />
      </NavProvider>,
    );
    await waitFor(() => expect(screen.getByText(/migration deadlocks/)).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: /Delete assist/ }));
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByText(/everything shared on it/)).toBeNull();
    expect(fetchMock.mock.calls.every((c) => c[1]?.method !== "DELETE")).toBe(true);
  });
});
