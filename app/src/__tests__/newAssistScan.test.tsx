import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { NavProvider } from "../app/router";
import { NewAssist } from "../screens/NewAssist";
import { mockHubFetch } from "./fixtures";

// Simulate what the agent module's live scan reports on a machine with one
// terminal session and its working directory.
vi.mock("../api/agent", () => ({
  suggestArtifacts: vi.fn(async () => [
    {
      title: "Terminals",
      items: [
        {
          id: "t-ttys001",
          kind: "terminal",
          badge: "iT",
          label: "iTerm2 (ttys001)",
          detail: "/work/payments",
          warn: false,
          icon: "data:image/png;base64,QUJD",
          pid: 4211,
        },
      ],
    },
    {
      title: "Files",
      items: [
        {
          id: "d-1",
          kind: "file",
          badge: "DIR",
          label: "payments",
          detail: "/work/payments",
          warn: false,
          icon: null,
          pid: null,
        },
      ],
    },
  ]),
  envFingerprint: vi.fn(async () => ["macos aarch64"]),
  snapshotPaths: vi.fn(async () => null),
}));

function renderScreen() {
  return render(
    <NavProvider>
      <NewAssist />
    </NavProvider>,
  );
}

describe("New assist picker with a live scan", () => {
  it("shows scanned artifacts inside the dialog, grouped by category", async () => {
    mockHubFetch();
    const { container } = renderScreen();
    await userEvent.click(screen.getByRole("button", { name: /Add artifacts/ }));

    // Terminals tab is active by default and lists the scanned session
    // with its real app icon.
    await waitFor(() => expect(screen.getByText("iTerm2 (ttys001)")).toBeTruthy());
    expect(container.querySelectorAll('img[src^="data:image/png;base64,"]').length).toBe(1);

    // Files tab lists the scanned working directory as a suggested path.
    await userEvent.click(screen.getByRole("button", { name: /Files & Directories/ }));
    expect(screen.getByText("payments")).toBeTruthy();
    expect(screen.getByText("Suggested")).toBeTruthy();
  });

  it("selecting in the dialog updates the page's artifact list", async () => {
    mockHubFetch();
    const { container } = renderScreen();
    await userEvent.click(screen.getByRole("button", { name: /Add artifacts/ }));
    await waitFor(() => expect(screen.getByText("iTerm2 (ttys001)")).toBeTruthy());

    await userEvent.click(screen.getByText("iTerm2 (ttys001)"));
    expect(screen.getByText(/1 selected for analysis/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));

    await waitFor(() => {
      expect(container.textContent).toContain("Artifacts - 1 selected");
      expect(screen.getByText("iTerm2 (ttys001)")).toBeTruthy();
    });
  });
});
