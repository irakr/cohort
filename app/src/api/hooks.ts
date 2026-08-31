import { useCallback, useEffect, useRef, useState } from "react";
import { apiGet } from "./client";

export interface ApiState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * Fetch a hub resource. `path: null` disables the fetch. `pollMs` keeps the
 * resource fresh (the base version has no WebSockets; the assist detail polls
 * so approvals and live-debug grants propagate).
 */
export function useApi<T>(
  path: string | null,
  opts: { pollMs?: number; deps?: unknown[] } = {},
): ApiState<T> {
  const { pollMs, deps = [] } = opts;
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(path !== null);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);

  const load = useCallback(
    async (background: boolean) => {
      if (path === null) {
        return;
      }
      const gen = ++generation.current;
      if (!background) {
        setLoading(true);
      }
      try {
        const result = await apiGet<T>(path);
        if (gen === generation.current) {
          setData(result);
          setError(null);
        }
      } catch (e) {
        if (gen === generation.current && !background) {
          setError(e instanceof Error ? e.message : String(e));
        }
      } finally {
        if (gen === generation.current && !background) {
          setLoading(false);
        }
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [path, ...deps],
  );

  useEffect(() => {
    setData(null);
    setError(null);
    if (path === null) {
      setLoading(false);
      return;
    }
    void load(false);
  }, [load, path]);

  useEffect(() => {
    if (path === null || !pollMs) {
      return;
    }
    const timer = setInterval(() => void load(true), pollMs);
    return () => clearInterval(timer);
  }, [load, path, pollMs]);

  const refetch = useCallback(() => void load(false), [load]);
  return { data, loading, error, refetch };
}
