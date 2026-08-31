import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "../App";
import { mockHubFetch } from "./fixtures";

describe("First-launch identity", () => {
  it("shows setup when the machine has no identity, and registers a new user", async () => {
    localStorage.clear();
    const fetchMock = mockHubFetch();
    render(<App />);

    // Setup screen, not the assists list.
    await waitFor(() => expect(screen.getByText(/Connect to your team's hub/)).toBeTruthy());

    await userEvent.type(screen.getByPlaceholderText("Your name"), "Ira K.");
    await userEvent.click(screen.getByRole("button", { name: "Join" }));

    // Identity persisted and the app proper is shown.
    await waitFor(() => expect(localStorage.getItem("cohort.userId")).toBe("u-ira-k"));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    const registerCall = fetchMock.mock.calls.find(
      (c) => String(c[0]).endsWith("/api/users") && c[1]?.method === "POST",
    );
    expect(registerCall).toBeTruthy();
  });

  it("continues as an existing user without any register call", async () => {
    localStorage.clear();
    const fetchMock = mockHubFetch();
    render(<App />);

    await waitFor(() => expect(screen.getByRole("button", { name: /Meera N\./ })).toBeTruthy());
    await userEvent.click(screen.getByRole("button", { name: /Meera N\./ }));

    await waitFor(() => expect(localStorage.getItem("cohort.userId")).toBe("u-meera"));
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    expect(
      fetchMock.mock.calls.some(
        (c) => String(c[0]).endsWith("/api/users") && c[1]?.method === "POST",
      ),
    ).toBe(false);
  });

  it("skips setup when the machine already has an identity", async () => {
    mockHubFetch();
    render(<App />);
    await waitFor(() => expect(screen.getByText("Assists")).toBeTruthy());
    expect(screen.queryByText(/Connect to your team's hub/)).toBeNull();
  });
});
