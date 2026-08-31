import { getCurrentUserId } from "./currentUser";
import { getHubUrl } from "./hubUrl";

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  const userId = getCurrentUserId();
  if (userId) {
    headers["X-User-Id"] = userId;
  }
  if (body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  const response = await fetch(`${getHubUrl()}${path}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    let message = `${response.status}`;
    try {
      const payload = await response.json();
      if (payload && typeof payload.error === "string") {
        message = payload.error;
      }
    } catch {
      // non-JSON error body
    }
    throw new ApiError(response.status, message);
  }
  return (await response.json()) as T;
}

export function apiGet<T>(path: string): Promise<T> {
  return request<T>("GET", path);
}

export function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return request<T>("POST", path, body ?? {});
}
