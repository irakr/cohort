// This machine's identity: chosen once on first launch (register or pick an
// existing user on the hub), then persisted. Real auth arrives with P2.

const KEY = "cohort.userId";

export function getCurrentUserId(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

export function setCurrentUserId(id: string): void {
  try {
    localStorage.setItem(KEY, id);
  } catch {
    // storage unavailable
  }
}

export function clearCurrentUserId(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // storage unavailable
  }
}
