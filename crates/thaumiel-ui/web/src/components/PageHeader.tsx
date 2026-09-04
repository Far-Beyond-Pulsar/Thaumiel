import { ReactNode } from "react";

export function PageHeader({ title, subtitle, action }: { title: string; subtitle?: string; action?: ReactNode }) {
  return (
    <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", marginBottom: 24, gap: 16 }}>
      <div>
        <h1 style={{ fontSize: 20, fontWeight: 600, margin: 0 }}>{title}</h1>
        {subtitle ? <p style={{ fontSize: 13, color: "var(--text-dim)", margin: "4px 0 0" }}>{subtitle}</p> : null}
      </div>
      {action}
    </div>
  );
}
