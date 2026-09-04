import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DraftOutcome, LlmConfig, Preset } from "../api/types";
import { NavProvider } from "../app/router";
import { Rail } from "../app/Rail";
import { NewAssist } from "../screens/NewAssist";
import { mockHubFetch } from "./fixtures";

// The assistant bridge as a machine with a configured model would answer.
const agent = vi.hoisted(() => ({
  assistantPresets: vi.fn(async (): Promise<Preset[]> => [
    {
      id: "anthropic",
      name: "Anthropic",
      protocol: "anthropic",
      base_url: "https://api.anthropic.com",
      default_model: "claude-opus-5",
      needs_key: true,
    },
    {
      id: "ollama",
      name: "Ollama (local)",
      protocol: "openai_compatible",
      base_url: "http://localhost:11434/v1",
      default_model: "",
      needs_key: false,
    },
  ]),
  assistantConfigGet: vi.fn(async (): Promise<LlmConfig | null> => ({
    protocol: "anthropic",
    base_url: "https://api.anthropic.com",
    api_key: "sk-test",
    model: "claude-opus-5",
  })),
  assistantConfigSet: vi.fn(async () => null),
  assistantConfigTest: vi.fn(async () => ({ ok: true, message: "Reached claude-opus-5 (9 tokens in, 1 out)." })),
  draftInsights: vi.fn(async (): Promise<DraftOutcome> => ({
    draft: { insights: "- intent: ship 1.9.4 to staging", environment: ["Kubernetes 1.29"] },
    note: null,
    model: "claude-opus-5",
    input_tokens: 812,
    output_tokens: 44,
  })),
  suggestArtifacts: vi.fn(async () => []),
  envFingerprint: vi.fn(async () => ["macos aarch64"]),
  snapshotPaths: vi.fn(async () => null),
  sshPublicKey: vi.fn(async () => null),
  sshTargetSuggestion: vi.fn(async () => ""),
  installSshKey: vi.fn(async () => false),
  openSsh: vi.fn(async () => null),
}));
vi.mock("../api/agent", () => agent);

describe("Assistant: insights on New assist", () => {
  it("says which model drafts here, drafts on this machine, and posts the result", async () => {
    const fetchMock = mockHubFetch();
    render(
      <NavProvider>
        <NewAssist />
      </NavProvider>,
    );
    await waitFor(() =>
      expect(screen.getByText(/Insights: drafted by claude-opus-5 on this machine/)).toBeTruthy(),
    );

    await userEvent.type(screen.getByPlaceholderText("One line on what is stuck"), "Rollout hangs on an image pull");
    await userEvent.type(
      screen.getByPlaceholderText("Describe the problem in your own words. Markdown works."),
      "The pod never becomes ready.",
    );
    await userEvent.click(screen.getByRole("button", { name: "Create assist" }));

    // Drafted locally from the owner's words (no artifacts selected here).
    await waitFor(() => expect(agent.draftInsights).toHaveBeenCalled());
    expect(agent.draftInsights).toHaveBeenCalledWith({
      title: "Rollout hangs on an image pull",
      description: "The pod never becomes ready.",
      artifacts: [],
    });

    // The hub receives the finished assist; it never drafted anything.
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some((c) => String(c[0]).endsWith("/api/assists") && c[1]?.method === "POST"),
      ).toBe(true),
    );
    const create = fetchMock.mock.calls.find(
      (c) => String(c[0]).endsWith("/api/assists") && c[1]?.method === "POST",
    );
    const body = JSON.parse(String(create?.[1]?.body)) as { insights: string; environment: string[] };
    expect(body.insights).toBe("- intent: ship 1.9.4 to staging");
    // Model chips first, then this machine's fingerprint, no duplicates.
    expect(body.environment).toEqual(["Kubernetes 1.29", "macos aarch64"]);
    expect(fetchMock.mock.calls.some((c) => String(c[0]).includes("draft-brief"))).toBe(false);
  });

  it("without a configured assistant the assist is created with empty insights", async () => {
    agent.assistantConfigGet.mockResolvedValueOnce(null);
    agent.draftInsights.mockResolvedValueOnce({
      draft: { insights: "", environment: [] },
      note: "No assistant is configured on this machine.",
      model: null,
      input_tokens: 0,
      output_tokens: 0,
    });
    const fetchMock = mockHubFetch();
    render(
      <NavProvider>
        <NewAssist />
      </NavProvider>,
    );
    await waitFor(() => expect(screen.getByText(/no assistant configured on this machine/)).toBeTruthy());

    await userEvent.type(screen.getByPlaceholderText("One line on what is stuck"), "Anything");
    await userEvent.click(screen.getByRole("button", { name: "Create assist" }));
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some((c) => String(c[0]).endsWith("/api/assists") && c[1]?.method === "POST"),
      ).toBe(true),
    );
    const create = fetchMock.mock.calls.find(
      (c) => String(c[0]).endsWith("/api/assists") && c[1]?.method === "POST",
    );
    const body = JSON.parse(String(create?.[1]?.body)) as { insights: string; environment: string[] };
    expect(body.insights).toBe("");
    expect(body.environment).toEqual(["macos aarch64"]);
  });
});

describe("Assistant: settings", () => {
  it("shows the saved configuration and saves a preset-filled one", async () => {
    mockHubFetch();
    render(
      <NavProvider>
        <Rail />
      </NavProvider>,
    );
    await userEvent.click(screen.getByTitle("Settings"));
    await waitFor(() => expect(screen.getByText("Assistant")).toBeTruthy());

    // The saved configuration is shown, preset recognised from its URL.
    const preset = screen.getByLabelText("Provider") as HTMLSelectElement;
    await waitFor(() => expect(preset.value).toBe("anthropic"));
    expect((screen.getByLabelText("Model") as HTMLInputElement).value).toBe("claude-opus-5");
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();

    // Switch to a local server: URL comes from the preset, model is typed.
    await userEvent.selectOptions(preset, "ollama");
    expect((screen.getByLabelText("Base URL") as HTMLInputElement).value).toBe("http://localhost:11434/v1");
    expect(screen.getByPlaceholderText("not needed for this provider")).toBeTruthy();
    await userEvent.clear(screen.getByLabelText("Model"));
    await userEvent.type(screen.getByLabelText("Model"), "llama3");
    await userEvent.clear(screen.getByLabelText("API key"));

    await userEvent.click(screen.getByRole("button", { name: "Test" }));
    await waitFor(() => expect(screen.getByText(/Reached claude-opus-5/)).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: "Save assistant" }));
    await waitFor(() => expect(agent.assistantConfigSet).toHaveBeenCalled());
    expect(agent.assistantConfigSet).toHaveBeenLastCalledWith({
      protocol: "openai_compatible",
      base_url: "http://localhost:11434/v1",
      model: "llama3",
      api_key: null,
    });
    expect(screen.getByText(/Saved\. Insights are drafted on this machine/)).toBeTruthy();
  });

  it("cannot save without a URL and a model, and Remove forgets the configuration", async () => {
    agent.assistantConfigGet.mockResolvedValueOnce(null);
    mockHubFetch();
    render(
      <NavProvider>
        <Rail />
      </NavProvider>,
    );
    await userEvent.click(screen.getByTitle("Settings"));
    await waitFor(() => expect(screen.getByText("Assistant")).toBeTruthy());

    const save = screen.getByRole("button", { name: "Save assistant" }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();

    await userEvent.selectOptions(screen.getByLabelText("Provider"), "anthropic");
    expect((screen.getByLabelText("Model") as HTMLInputElement).value).toBe("claude-opus-5");
    expect(save.disabled).toBe(false);
    await userEvent.click(save);
    await waitFor(() => expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy());

    await userEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(agent.assistantConfigSet).toHaveBeenLastCalledWith(null));
    expect((screen.getByLabelText("Model") as HTMLInputElement).value).toBe("");
    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();
  });
});
