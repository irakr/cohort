import { createContext, useCallback, useContext, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { clearCurrentUserId, getCurrentUserId, setCurrentUserId } from "../api/currentUser";

export type Screen =
  | { name: "assists" }
  | { name: "new" }
  | { name: "assist"; ref: string }
  | { name: "close"; ref: string }
  | { name: "record" };

interface Nav {
  screen: Screen;
  navigate: (screen: Screen) => void;
  /** null until the machine's identity is set on the setup screen. */
  currentUserId: string | null;
  setUser: (id: string) => void;
  signOut: () => void;
}

const NavContext = createContext<Nav | null>(null);

export function NavProvider({ children }: { children: ReactNode }) {
  const [screen, setScreen] = useState<Screen>({ name: "assists" });
  const [currentUserId, setUserState] = useState<string | null>(getCurrentUserId());

  const navigate = useCallback((next: Screen) => setScreen(next), []);
  const setUser = useCallback((id: string) => {
    setCurrentUserId(id);
    setUserState(id);
    setScreen({ name: "assists" });
  }, []);
  const signOut = useCallback(() => {
    clearCurrentUserId();
    setUserState(null);
    setScreen({ name: "assists" });
  }, []);

  const value = useMemo(
    () => ({ screen, navigate, currentUserId, setUser, signOut }),
    [screen, navigate, currentUserId, setUser, signOut],
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
