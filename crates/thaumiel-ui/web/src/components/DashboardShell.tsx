"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useAppState } from "@/lib/app-state";
import { Sidebar } from "./Sidebar";
import styles from "./DashboardShell.module.css";

const COLLAPSE_KEY = "thaumiel.sidebar-collapsed";

export function DashboardShell({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { ready, identity, api, apiBaseUrl } = useAppState();
  const [collapsed, setCollapsed] = useState(false);
  const [orgName, setOrgName] = useState<string | null>(null);

  useEffect(() => {
    const stored = window.localStorage.getItem(COLLAPSE_KEY);
    if (stored) setCollapsed(stored === "1");
  }, []);

  useEffect(() => {
    if (ready && !identity) {
      router.replace("/login");
    }
  }, [ready, identity, router]);

  useEffect(() => {
    if (!identity) return;
    api.me().then((org) => setOrgName(org.name)).catch(() => setOrgName(null));
  }, [identity, api]);

  const toggle = () => {
    setCollapsed((prev) => {
      window.localStorage.setItem(COLLAPSE_KEY, prev ? "0" : "1");
      return !prev;
    });
  };

  if (!ready || !identity) {
    return <div className={styles.loading}>Loading…</div>;
  }

  return (
    <div className={styles.layout}>
      <Sidebar collapsed={collapsed} onToggle={toggle} />
      <div className={styles.main}>
        <header className={styles.topbar}>
          <span className={styles.orgName}>{orgName ?? " "}</span>
          <span className={styles.apiTag}>
            <span className={styles.apiDot} />
            {apiBaseUrl}
          </span>
        </header>
        <main className={styles.content}>{children}</main>
      </div>
    </div>
  );
}
