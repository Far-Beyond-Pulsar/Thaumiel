"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { ApiClient } from "./api";
import { DEFAULT_API_BASE_URL, loadRuntimeConfig } from "./runtime-config";
import type { Identity } from "./types";

const STORAGE_KEY = "thaumiel.session";

interface StoredSession {
  token: string;
  identity: Identity;
}

interface AppState {
  /** True until the runtime config fetch (and the localStorage read) resolve. */
  ready: boolean;
  apiBaseUrl: string;
  identity: Identity | null;
  api: ApiClient;
  /** An unauthenticated client bound to the same apiBaseUrl -- for /auth/*. */
  publicApi: ApiClient;
  setSession: (session: StoredSession) => void;
  clearSession: () => void;
}

const AppStateContext = createContext<AppState | null>(null);

export function AppStateProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  const [apiBaseUrl, setApiBaseUrl] = useState(DEFAULT_API_BASE_URL);
  const [session, setSessionState] = useState<StoredSession | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const config = await loadRuntimeConfig();
      const raw = window.localStorage.getItem(STORAGE_KEY);
      const stored = raw ? (JSON.parse(raw) as StoredSession) : null;
      if (!cancelled) {
        setApiBaseUrl(config.apiBaseUrl);
        setSessionState(stored);
        setReady(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const setSession = useCallback((next: StoredSession) => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    setSessionState(next);
  }, []);

  const clearSession = useCallback(() => {
    window.localStorage.removeItem(STORAGE_KEY);
    setSessionState(null);
  }, []);

  const api = useMemo(() => new ApiClient(apiBaseUrl, session?.token ?? null), [apiBaseUrl, session]);
  const publicApi = useMemo(() => new ApiClient(apiBaseUrl, null), [apiBaseUrl]);

  const value: AppState = {
    ready,
    apiBaseUrl,
    identity: session?.identity ?? null,
    api,
    publicApi,
    setSession,
    clearSession,
  };

  return <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>;
}

export function useAppState(): AppState {
  const ctx = useContext(AppStateContext);
  if (!ctx) throw new Error("useAppState must be used inside <AppStateProvider>");
  return ctx;
}
