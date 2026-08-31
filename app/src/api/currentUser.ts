// Seeded-user selection (real auth arrives with P2). The id rides on every
// request as X-User-Id; the switcher in the rail footer changes it.

const KEY = "cohort.userId";
const DEFAULT_ID = "u-alex";

export function getCurrentUserId(): string {
  try {
    return localStorage.getItem(KEY) || DEFAULT_ID;
  } catch {
    return DEFAULT_ID;
  }
}

export function setCurrentUserId(id: string): void {
  try {
    localStorage.setItem(KEY, id);
  } catch {
    // storage unavailable
  }
}
