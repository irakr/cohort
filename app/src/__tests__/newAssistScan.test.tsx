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
}));

function renderScreen() {
  return render(
    <NavProvider>
      <NewAssist />
    </NavProvider>,
  );
}

describe("New assist picker with a live scan", () => {
  it("renders the scanned terminals and directories as suggestions", async () => {
    mockHubFetch();
    const { container } = renderScreen();
    await waitFor(() => expect(screen.getByText("iTerm2 (ttys001)")).toBeTruthy());
    expect(screen.getByText("payments")).toBeTruthy();
    expect(screen.getAllByText("/work/payments").length).toBe(2);

    // The terminal shows its real app icon; the directory has no app icon
    // and renders the folder glyph placeholder instead of an <img>.
    const icons = container.querySelectorAll('img[src^="data:image/png;base64,"]');
    expect(icons.length).toBe(1);

    await userEvent.click(screen.getByText("iTerm2 (ttys001)"));
    await waitFor(() => expect(container.textContent).toContain("For analysis - 1 item"));
  });

  it("the Add artifacts wizard re-scans and hides already-listed candidates", async () => {
    mockHubFetch();
    renderScreen();
    await waitFor(() => expect(screen.getByText("iTerm2 (ttys001)")).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: "Add artifacts" }));
    await userEvent.click(screen.getByRole("button", { name: /Add a terminal/ }));

    // The scanned terminal is already suggested, so the wizard offers only
    // manual entry for this type.
    await waitFor(() =>
      expect(screen.getByText(/Nothing new detected for this type right now/)).toBeTruthy(),
    );
    expect(screen.getByPlaceholderText("terminal name, e.g. iTerm2")).toBeTruthy();
  });
});
