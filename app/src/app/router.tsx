import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { getCurrentUserId, setCurrentUserId } from "../api/currentUser";

export type Screen =
  | { name: "assists" }
  | { name: "new" }
  | { name: "assist"; ref: string }
  | { name: "close"; ref: string }
  | { name: "record" };

interface Nav {
  screen: Screen;
  navigate: (screen: Screen) => void;
  currentUserId: string;
  setUser: (id: string) => void;
}

const NavContext = createContext<Nav | null>(null);

export function NavProvider({ children }: { children: ReactNode }) {
  const [screen, setScreen] = useState<Screen>({ name: "assists" });
  const [currentUserId, setUserState] = useState(getCurrentUserId());

  const navigate = useCallback((next: Screen) => setScreen(next), []);
  const setUser = useCallback((id: string) => {
    setCurrentUserId(id);
    setUserState(id);
  }, []);

  const value = useMemo(
    () => ({ screen, navigate, currentUserId, setUser }),
    [screen, navigate, currentUserId, setUser],
  );
  return <NavContext.Provider value={value}>{children}</NavContext.Provider>;
}

export function useNav(): Nav {
  const nav = useContext(NavContext);
  if (!nav) {
    throw new Error("useNav outside NavProvider");
  }
  return nav;
}
