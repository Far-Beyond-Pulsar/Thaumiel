import { ReactNode } from "react";
import styles from "./AuthCard.module.css";

export function AuthCard({
  title,
  subtitle,
  error,
  children,
  footer,
}: {
  title: string;
  subtitle?: string;
  error?: string | null;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <div className={styles.brand}>
          <div className={styles.mark}>T</div>
          <span className={styles.wordmark}>Thaumiel</span>
        </div>
        <div className={styles.panel}>
          <div>
            <p className={styles.title}>{title}</p>
            {subtitle ? <p className={styles.subtitle}>{subtitle}</p> : null}
          </div>
          {error ? <div className={styles.error}>{error}</div> : null}
          {children}
        </div>
        {footer ? <div className={styles.footerLink}>{footer}</div> : null}
      </div>
    </div>
  );
}
