import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { apiGet } from "../api/client";
import { clearCurrentUserId, getCurrentUserId, setCurrentUserId } from "../api/currentUser";
import type { User } from "../api/types";

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
  /** False while a stored identity is still being checked against the hub.
      Nothing that needs an identity may render until this is true. */
  identityChecked: boolean;
  setUser: (id: string) => void;
  signOut: () => void;
}

const NavContext = createContext<Nav | null>(null);

export function NavProvider({ children }: { children: ReactNode }) {
  const [screen, setScreen] = useState<Screen>({ name: "assists" });
  const [currentUserId, setUserState] = useState<string | null>(getCurrentUserId());
  // An identity restored from this machine is unverified; one we just got
  // from the hub is not.
  const [identityChecked, setIdentityChecked] = useState(getCurrentUserId() === null);

  const navigate = useCallback((next: Screen) => setScreen(next), []);
  const setUser = useCallback((id: string) => {
    setCurrentUserId(id);
    setUserState(id);
    setIdentityChecked(true);
    setScreen({ name: "assists" });
  }, []);
  const signOut = useCallback(() => {
    clearCurrentUserId();
    setUserState(null);
    setIdentityChecked(true);
    setScreen({ name: "assists" });
  }, []);

  // The hub is the authority on who exists. A stored identity the hub does
  // not know (its database was reset, or this is a different hub) would leave
  // every request failing with "unknown user", so drop back to Setup and let
  // the machine register again. A hub that cannot be reached at all is not a
  // rejection: the identity stays.
  useEffect(() => {
    if (!currentUserId || identityChecked) {
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const users = await apiGet<User[]>("/api/users");
        if (cancelled) {
          return;
        }
        if (!users.some((u) => u.id === currentUserId)) {
          console.warn(`hub does not know ${currentUserId}; returning to setup`);
          clearCurrentUserId();
          setUserState(null);
        }
      } catch {
        // Hub unreachable; keep the identity and let the screens report it.
      } finally {
        if (!cancelled) {
          setIdentityChecked(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [currentUserId, identityChecked]);

  const value = useMemo(
    () => ({ screen, navigate, currentUserId, identityChecked, setUser, signOut }),
    [screen, navigate, currentUserId, identityChecked, setUser, signOut],
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
