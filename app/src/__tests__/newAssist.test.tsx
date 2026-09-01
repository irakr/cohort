import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "../App";
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
  it("starts with no artifacts and Create disabled", async () => {
    mockHubFetch();
    renderScreen();
    await waitFor(() => expect(screen.getByText(/No artifacts yet/)).toBeTruthy());
    expect(screen.getByText(/None selected/)).toBeTruthy();
    const create = screen.getByRole("button", { name: "Create assist" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
  });

  it("adds a custom path via the dialog; a title enables Create", async () => {
    mockHubFetch();
    const { container } = renderScreen();
    await waitFor(() => expect(screen.getByText(/No artifacts yet/)).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: /Add artifacts/ }));
    await userEvent.click(screen.getByRole("button", { name: /Files & Directories/ }));
    await userEvent.type(
      screen.getByPlaceholderText("path/to/file or directory/"),
      "k8s/payments/deployment.yaml",
    );
    await userEvent.click(screen.getByRole("button", { name: "Add path" }));
    // Selected count shows in the dialog footer, then on the page card.
    expect(screen.getByText(/1 selected for analysis/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));

    await waitFor(() => {
      expect(container.textContent).toContain("Artifacts - 1 selected");
      expect(screen.getByText("deployment.yaml")).toBeTruthy();
    });

    const create = screen.getByRole("button", { name: "Create assist" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    await userEvent.type(
      screen.getByPlaceholderText("One line on what is stuck"),
      "Rollout stuck on image pull",
    );
    expect(create.disabled).toBe(false);
  });

  it("cancel leaves the wizard and returns to the assists list", async () => {
    mockHubFetch();
    render(<App />);
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: "Open an assist" }));
    await waitFor(() => expect(screen.getByText("New assist")).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    expect(screen.queryByText("New assist")).toBeNull();
  });

  it("collects free-text tags as removable chips", async () => {
    mockHubFetch();
    renderScreen();
    const input = screen.getByPlaceholderText("type a tag, press Enter");
    await userEvent.type(input, "kubernetes{enter}");
    await userEvent.type(input, "Registry Auth{enter}");

    expect(screen.getByText("kubernetes")).toBeTruthy();
    expect(screen.getByText("registry-auth")).toBeTruthy();

    await userEvent.click(screen.getByText("kubernetes"));
    expect(screen.queryByText("kubernetes")).toBeNull();
  });
});
