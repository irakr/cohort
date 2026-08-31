import { afterEach, beforeEach, vi } from "vitest";

beforeEach(() => {
  // Component tests run as an already-identified machine; the setup-screen
  // tests clear this themselves.
  localStorage.setItem("cohort.userId", "u-alex");
});

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});
