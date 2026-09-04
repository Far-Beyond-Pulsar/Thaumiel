"use client";

import { useEffect, useState } from "react";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { Card, CardHeader } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { useAppState } from "@/lib/app-state";
import type { KeygenBackendInfo, Organization } from "@/lib/types";

export default function SettingsPage() {
  const { api, apiBaseUrl, identity } = useAppState();
  const [org, setOrg] = useState<Organization | null>(null);
  const [backends, setBackends] = useState<KeygenBackendInfo[]>([]);

  useEffect(() => {
    if (!identity) return;
    api.me().then(setOrg);
    api.keygenBackends().then(setBackends);
  }, [api, identity]);

  return (
    <DashboardShell>
      <PageHeader title="Settings" subtitle="Connection and server-reported information." />

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <Card>
          <CardHeader title="Connection" />
          <dl style={{ display: "grid", gridTemplateColumns: "160px 1fr", rowGap: 10, fontSize: 13, margin: 0 }}>
            <dt style={{ color: "var(--text-dim)" }}>API base URL</dt>
            <dd className="mono" style={{ margin: 0 }}>
              {apiBaseUrl}
            </dd>
            <dt style={{ color: "var(--text-dim)" }}>Organization</dt>
            <dd style={{ margin: 0 }}>{org?.name ?? "—"}</dd>
            <dt style={{ color: "var(--text-dim)" }}>Organization ID</dt>
            <dd className="mono" style={{ margin: 0 }}>
              {identity?.org_id}
            </dd>
            <dt style={{ color: "var(--text-dim)" }}>Signed in as</dt>
            <dd style={{ margin: 0 }}>
              {identity?.email} ({identity?.role})
            </dd>
          </dl>
        </Card>

        <Card>
          <CardHeader title="Keygen backends" subtitle="Every license key format the connected server has linked in." />
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {backends.map((b) => (
              <div key={b.id} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
                <div>
                  <div className="mono" style={{ fontSize: 13, fontWeight: 600 }}>
                    {b.id}
                  </div>
                  <div style={{ fontSize: 12, color: "var(--text-dim)" }}>{b.description}</div>
                </div>
                {b.offline_verifiable && <Badge tone="accent">offline-verifiable</Badge>}
              </div>
            ))}
          </div>
        </Card>
      </div>
    </DashboardShell>
  );
}
