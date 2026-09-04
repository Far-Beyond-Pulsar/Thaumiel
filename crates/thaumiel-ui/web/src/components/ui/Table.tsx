import { ReactNode } from "react";
import styles from "./Table.module.css";

export function Table({ children }: { children: ReactNode }) {
  return (
    <div className={styles.wrap}>
      <table className={styles.table}>{children}</table>
    </div>
  );
}

export function EmptyState({ label }: { label: string }) {
  return (
    <tr>
      <td colSpan={99} className={styles.empty}>
        {label}
      </td>
    </tr>
  );
}
