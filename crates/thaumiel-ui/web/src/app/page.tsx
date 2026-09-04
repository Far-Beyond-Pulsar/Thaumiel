"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { StatCard, StatGrid } from "@/components/StatCard";
import { UsageChart } from "@/components/UsageChart";
import { Card, CardHeader } from "@/components/ui/Card";
import { useAppState } from "@/lib/app-state";
import type { LicenseKey, UsageSummary } from "@/lib/types";

export default function DashboardPage() {
  const { api, identity } = useAppState();
  const [usage, setUsage] = useState<UsageSummary | null>(null);
  const [recent, setRecent] = useState<LicenseKey[]>([]);

  useEffect(() => {
    if (!identity) return;
    api.usage().then(setUsage);
    api.listLicenses().then((licenses) => setRecent(licenses.slice(0, 5)));
  }, [api, identity]);

  return (
    <DashboardShell>
      <PageHeader title="Dashboard" subtitle="An overview of your organization." />

      <StatGrid>
        <StatCard label="Products" value={usage?.products ?? "–"} />
        <StatCard label="Licenses" value={usage?.licenses_total ?? "–"} />
        <StatCard label="Active licenses" value={usage?.licenses_active ?? "–"} />
        <StatCard label="Active API keys" value={usage?.api_keys_active ?? "–"} />
      </StatGrid>

      <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1.3fr) minmax(0, 1fr)", gap: 16, alignItems: "start" }}>
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
              Nothing yet. Head to <Link href="/products" style={{ color: "var(--accent-text)" }}>Products</Link> to create
              one, then generate a license against it.
            </p>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {recent.map((l) => (
                <div key={l.id} style={{ display: "flex", justifyContent: "space-between", gap: 10, fontSize: 13, minWidth: 0 }}>
                  <span
                    className="mono"
                    style={{ color: "var(--text-dim)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", minWidth: 0 }}
                  >
                    {l.key}
                  </span>
                  <span style={{ color: "var(--text-faint)", flexShrink: 0 }}>
                    {l.seats} seat{l.seats === 1 ? "" : "s"}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card>
          <CardHeader title="Validate calls" subtitle="Last 14 days, this organization." />
          {usage ? <UsageChart days={usage.validate_calls_last_14_days} /> : null}
        </Card>
      </div>
    </DashboardShell>
  );
}
