// The hub runs on a commonly accessible server (or locally in dev), so its
// URL is a runtime setting, not a build-time constant.

const KEY = "cohort.hubUrl";
const DEFAULT_URL = "http://127.0.0.1:7400";

export function getHubUrl(): string {
  try {
    return localStorage.getItem(KEY) || DEFAULT_URL;
  } catch {
    return DEFAULT_URL;
  }
}

export function setHubUrl(url: string): void {
  try {
    const trimmed = url.trim().replace(/\/+$/, "");
    if (trimmed) {
      localStorage.setItem(KEY, trimmed);
    } else {
      localStorage.removeItem(KEY);
    }
  } catch {
    // storage unavailable; keep the default
  }
}
