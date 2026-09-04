"use client";

import { useEffect, useState } from "react";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { Card } from "@/components/ui/Card";
import { EmptyState, Table } from "@/components/ui/Table";
import { useAppState } from "@/lib/app-state";
import type { AuditLogEntry } from "@/lib/types";

export default function AuditLogPage() {
  const { api, identity } = useAppState();
  const [entries, setEntries] = useState<AuditLogEntry[] | null>(null);

  useEffect(() => {
    if (!identity) return;
    api.listAuditLog().then(setEntries);
  }, [api, identity]);

  return (
    <DashboardShell>
      <PageHeader title="Audit Log" subtitle="Every mutating admin action, most recent first." />
      <Card>
        <Table>
          <thead>
            <tr>
              <th>When</th>
              <th>Actor</th>
              <th>Action</th>
              <th>Target</th>
            </tr>
          </thead>
          <tbody>
            {entries?.length === 0 && <EmptyState label="Nothing recorded yet." />}
            {entries?.map((e) => (
              <tr key={e.id}>
                <td style={{ whiteSpace: "nowrap" }}>{new Date(e.created_at).toLocaleString()}</td>
                <td className="mono">{e.actor}</td>
                <td>{e.action}</td>
                <td className="mono" style={{ color: "var(--text-dim)" }}>
                  {e.target}
                </td>
              </tr>
            ))}
          </tbody>
        </Table>
      </Card>
    </DashboardShell>
  );
}
