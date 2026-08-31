import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { NavProvider } from "../app/router";
import { NewAssist } from "../screens/NewAssist";
import { mockHubFetch } from "./fixtures";

function renderScreen() {
  return render(
    <NavProvider>
      <NewAssist />
    </NavProvider>,
  );
}

describe("New assist picker", () => {
  it("starts with nothing selected and Create disabled", async () => {
    mockHubFetch();
    renderScreen();
    expect(screen.getByText(/nothing selected/)).toBeTruthy();
    const create = screen.getByRole("button", { name: "Create assist" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
  });

  it("selecting artifacts updates the analysis card; a title enables Create", async () => {
    mockHubFetch();
    const { container } = renderScreen();

    // Groups stream in on timers; the terminals group lands first.
    await waitFor(() => expect(screen.getByText("iTerm2 (payments)")).toBeTruthy(), {
      timeout: 3000,
    });
    await userEvent.click(screen.getByText("iTerm2 (payments)"));
    await waitFor(() => {
      expect(container.textContent).toContain("For analysis - 1 item");
    });

    const create = screen.getByRole("button", { name: "Create assist" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    await userEvent.type(
      screen.getByPlaceholderText("Rollout stuck on image pull"),
      "Rollout stuck on image pull",
    );
    expect(create.disabled).toBe(false);
  });
});
