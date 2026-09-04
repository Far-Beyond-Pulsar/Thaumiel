import Link from "next/link";

export default function NotFound() {
  return (
    <div
      style={{
        minHeight: "100vh",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 10,
      }}
    >
      <p style={{ fontSize: 13, color: "var(--text-faint)" }}>404</p>
      <p style={{ fontSize: 14, color: "var(--text-dim)" }}>That page doesn&rsquo;t exist.</p>
      <Link href="/" style={{ fontSize: 13, color: "var(--accent-text)", fontWeight: 500 }}>
        Back to dashboard
      </Link>
    </div>
  );
}
