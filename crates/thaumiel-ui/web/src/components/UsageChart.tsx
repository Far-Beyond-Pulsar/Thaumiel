import type { UsageDayCount } from "@/lib/types";
import styles from "./UsageChart.module.css";

/** Plain divs sized by count -- no charting library, matches the rest of
 * the dashboard's flat, no-gradient design. */
export function UsageChart({ days }: { days: UsageDayCount[] }) {
  const max = Math.max(1, ...days.map((d) => d.count));

  return (
    <div className={styles.chart}>
      {days.map((d, i) => {
        // Only label every ~4th bar (always the last) -- 14 full date
        // labels crowd a narrow card; the tooltip carries the exact date.
        const showLabel = i === days.length - 1 || i % 4 === 0;
        return (
          <div key={d.date} className={styles.col} title={`${d.date}: ${d.count} validate call${d.count === 1 ? "" : "s"}`}>
            <div className={styles.bar} style={{ height: `${Math.max(4, (d.count / max) * 100)}%` }} />
            <span className={[styles.label, showLabel ? "" : styles.labelHidden].join(" ")}>
              {d.date.slice(5).replace("-", "/")}
            </span>
          </div>
        );
      })}
    </div>
  );
}
