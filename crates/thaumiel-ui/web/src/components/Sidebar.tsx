"use client";

import {
  Fingerprint,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Package,
  PanelLeftClose,
  PanelLeftOpen,
  ScrollText,
  Settings,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useAppState } from "@/lib/app-state";
import styles from "./Sidebar.module.css";

const NAV = [
  { href: "/", label: "Dashboard", icon: LayoutDashboard },
  { href: "/products", label: "Products", icon: Package },
  { href: "/licenses", label: "Licenses", icon: KeyRound },
  { href: "/api-keys", label: "API Keys", icon: Fingerprint },
  { href: "/audit-log", label: "Audit Log", icon: ScrollText },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function Sidebar({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const pathname = usePathname();
  const { identity, clearSession } = useAppState();

  return (
    <aside className={[styles.sidebar, collapsed ? styles.collapsed : ""].join(" ")}>
      <div className={styles.brand}>
        <div className={styles.mark}>T</div>
        <span className={styles.wordmark}>Thaumiel</span>
      </div>

      <nav className={styles.nav}>
        {NAV.map(({ href, label, icon: Icon }) => {
          const active = href === "/" ? pathname === "/" : pathname?.startsWith(href);
          return (
            <Link key={href} href={href} className={[styles.item, active ? styles.itemActive : ""].join(" ")}>
              <span className={styles.icon}>
                <Icon size={17} strokeWidth={1.9} />
              </span>
              <span className={styles.label}>{label}</span>
            </Link>
          );
        })}
      </nav>

      <div className={styles.footer}>
        <button className={styles.toggle} onClick={onToggle} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
          <span className={styles.icon}>
            {collapsed ? <PanelLeftOpen size={17} strokeWidth={1.9} /> : <PanelLeftClose size={17} strokeWidth={1.9} />}
          </span>
          <span className={styles.label}>Collapse</span>
        </button>

        <div className={styles.user}>
          <div className={styles.avatar}>{identity?.email.slice(0, 2).toUpperCase() ?? "--"}</div>
          <div className={styles.userText}>
            <span className={styles.userEmail}>{identity?.email ?? "unknown"}</span>
            <span className={styles.userRole}>{identity?.role ?? ""}</span>
          </div>
        </div>

        <button className={styles.toggle} onClick={clearSession} aria-label="Sign out">
          <span className={styles.icon}>
            <LogOut size={17} strokeWidth={1.9} />
          </span>
          <span className={styles.label}>Sign out</span>
        </button>
      </div>
    </aside>
  );
}
