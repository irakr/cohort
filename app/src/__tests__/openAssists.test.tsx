import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { NavProvider } from "../app/router";
import { OpenAssists } from "../screens/OpenAssists";
import { mockHubFetch } from "./fixtures";

function renderScreen() {
  return render(
    <NavProvider>
      <OpenAssists />
    </NavProvider>,
  );
}

describe("Open assists", () => {
  it("renders every assist with responder names, never counts", async () => {
    mockHubFetch();
    renderScreen();
    await waitFor(() => {
      expect(
        screen.getByText("Rollout hangs on an image pull"),
      ).toBeTruthy();
    });
    expect(screen.getAllByText(/^S-\d+$/).length).toBe(4);
    // Names, not a count.
    expect(screen.getByText("Responder, Bystander")).toBeTruthy();
    expect(screen.queryByText(/2 responders/)).toBeNull();
  });

  it("filters by status through hub query params", async () => {
    const fetchMock = mockHubFetch();
    renderScreen();
    await waitFor(() => expect(screen.getAllByText(/^S-\d+$/).length).toBe(4));

    await userEvent.click(screen.getByRole("button", { name: /dormant/ }));
    await waitFor(() => {
      expect(screen.getAllByText(/^S-\d+$/).length).toBe(1);
      expect(screen.getByText(/Vite build runs out of memory/)).toBeTruthy();
    });
    const urls = fetchMock.mock.calls.map((c) => String(c[0]));
    expect(urls.some((u) => u.includes("status=dormant"))).toBe(true);
  });

  it("scopes to my assists with the toggle", async () => {
    mockHubFetch();
    renderScreen();
    await waitFor(() => expect(screen.getAllByText(/^S-\d+$/).length).toBe(4));

    await userEvent.click(screen.getByRole("button", { name: /My assists/ }));
    await waitFor(() => {
      expect(screen.getAllByText(/^S-\d+$/).length).toBe(2);
      expect(screen.queryByText(/Rollout hangs/)).toBeNull();
    });
  });
});
