"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { FormEvent, useState } from "react";
import { ApiError } from "@/lib/api";
import { AuthCard } from "@/components/AuthCard";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Field";
import { useAppState } from "@/lib/app-state";

export default function RegisterPage() {
  const router = useRouter();
  const { publicApi, setSession } = useAppState();
  const [orgName, setOrgName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const session = await publicApi.register(orgName.trim(), email.trim(), password);
      setSession(session);
      router.push("/");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not reach the API.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCard
      title="Create an organization"
      subtitle="This also creates your first user, with the owner role."
      error={error}
      footer={
        <>
          Already set up? <Link href="/login">Sign in</Link>
        </>
      }
    >
      <form onSubmit={onSubmit} style={{ display: "flex", flexDirection: "column", gap: 14 }}>
        <Input label="Organization name" required value={orgName} onChange={(e) => setOrgName(e.target.value)} placeholder="Acme" />
        <Input
          label="Email"
          type="email"
          required
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@company.com"
        />
        <Input
          label="Password"
          type="password"
          required
          minLength={8}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          hint="At least 8 characters."
        />
        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? "Creating…" : "Create organization"}
        </Button>
      </form>
    </AuthCard>
  );
}
