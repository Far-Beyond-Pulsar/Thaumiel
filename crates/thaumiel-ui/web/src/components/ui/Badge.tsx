import styles from "./Badge.module.css";

type Tone = "neutral" | "success" | "warning" | "danger" | "accent";

export function Badge({ tone = "neutral", children, dot = true }: { tone?: Tone; children: React.ReactNode; dot?: boolean }) {
  return (
    <span className={[styles.badge, styles[tone]].join(" ")}>
      {dot ? <span className={styles.dot} /> : null}
      {children}
    </span>
  );
}

const statusTone: Record<string, Tone> = {
  active: "success",
  suspended: "warning",
  revoked: "danger",
  expired: "neutral",
};

export function StatusBadge({ status }: { status: string }) {
  return <Badge tone={statusTone[status] ?? "neutral"}>{status}</Badge>;
}
