"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { StatCard, StatGrid } from "@/components/StatCard";
import { Card, CardHeader } from "@/components/ui/Card";
import { useAppState } from "@/lib/app-state";
import type { LicenseKey } from "@/lib/types";

export default function DashboardPage() {
  const { api, identity } = useAppState();
  const [counts, setCounts] = useState<{ products: number; licenses: number; active: number; apiKeys: number } | null>(null);
  const [recent, setRecent] = useState<LicenseKey[]>([]);

  useEffect(() => {
    if (!identity) return;
    Promise.all([api.listProducts(), api.listLicenses(), api.listApiKeys()]).then(([products, licenses, apiKeys]) => {
      setCounts({
        products: products.length,
        licenses: licenses.length,
        active: licenses.filter((l) => l.status === "active").length,
        apiKeys: apiKeys.filter((k) => !k.revoked_at).length,
      });
      setRecent(licenses.slice(0, 5));
    });
  }, [api, identity]);

  return (
    <DashboardShell>
      <PageHeader title="Dashboard" subtitle="An overview of your organization." />

      <StatGrid>
        <StatCard label="Products" value={counts?.products ?? "–"} />
        <StatCard label="Licenses" value={counts?.licenses ?? "–"} />
        <StatCard label="Active licenses" value={counts?.active ?? "–"} />
        <StatCard label="Active API keys" value={counts?.apiKeys ?? "–"} />
      </StatGrid>

      <Card>
        <CardHeader
          title="Recently generated licenses"
          action={
            <Link href="/licenses" style={{ fontSize: 12, color: "var(--accent-text)", fontWeight: 500 }}>
              View all
            </Link>
          }
        />
        {recent.length === 0 ? (
          <p style={{ fontSize: 13, color: "var(--text-faint)" }}>
            Nothing yet. Head to <Link href="/products" style={{ color: "var(--accent-text)" }}>Products</Link> to create one,
            then generate a license against it.
          </p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {recent.map((l) => (
              <div key={l.id} style={{ display: "flex", justifyContent: "space-between", fontSize: 13 }}>
                <span className="mono" style={{ color: "var(--text-dim)" }}>
                  {l.key.length > 42 ? `${l.key.slice(0, 42)}…` : l.key}
                </span>
                <span style={{ color: "var(--text-faint)" }}>{l.seats} seat{l.seats === 1 ? "" : "s"}</span>
              </div>
            ))}
          </div>
        )}
      </Card>
    </DashboardShell>
  );
}
