"use client";

import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { Modal } from "@/components/ui/Modal";
import { Badge } from "@/components/ui/Badge";
import { EmptyState, Table } from "@/components/ui/Table";
import { ApiError } from "@/lib/api";
import { useAppState } from "@/lib/app-state";
import type { Role, User } from "@/lib/types";

export default function TeamPage() {
  const { api, identity } = useAppState();
  const [users, setUsers] = useState<User[] | null>(null);
  const [open, setOpen] = useState(false);

  const canInvite = identity?.role === "owner" || identity?.role === "admin";

  function refresh() {
    api.listUsers().then(setUsers);
  }

  useEffect(() => {
    if (!identity) return;
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [identity]);

  return (
    <DashboardShell>
      <PageHeader
        title="Team"
        subtitle="Everyone with dashboard access to this organization."
        action={
          canInvite ? (
            <Button variant="primary" onClick={() => setOpen(true)}>
              <Plus size={15} /> Add user
            </Button>
          ) : null
        }
      />

      <Card>
        <Table>
          <thead>
            <tr>
              <th>Email</th>
              <th>Role</th>
              <th>Added</th>
            </tr>
          </thead>
          <tbody>
            {users?.length === 0 && <EmptyState label="No users yet." />}
            {users?.map((u) => (
              <tr key={u.id}>
                <td>{u.email}</td>
                <td>
                  <Badge tone={u.role === "owner" ? "accent" : "neutral"} dot={false}>
                    {u.role}
                  </Badge>
                </td>
                <td>{new Date(u.created_at).toLocaleDateString()}</td>
              </tr>
            ))}
          </tbody>
        </Table>
      </Card>

      {open && (
        <AddUserModal
          onClose={() => setOpen(false)}
          onCreated={() => {
            setOpen(false);
            refresh();
          }}
        />
      )}
    </DashboardShell>
  );
}

function AddUserModal({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const { api } = useAppState();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState<Role>("member");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      await api.createUser(email.trim(), password, role);
      onCreated();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not add the user.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="Add a user"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} disabled={busy || !email.trim() || password.length < 8}>
            {busy ? "Adding…" : "Add"}
          </Button>
        </>
      }
    >
      <p style={{ fontSize: 12, color: "var(--text-dim)" }}>
        There&rsquo;s no invite-email flow yet — share this temporary password with them directly.
      </p>
      {error ? <p style={{ color: "var(--danger)", fontSize: 12 }}>{error}</p> : null}
      <Input label="Email" type="email" autoFocus value={email} onChange={(e) => setEmail(e.target.value)} placeholder="teammate@company.com" />
      <Input
        label="Temporary password"
        type="text"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        hint="At least 8 characters. Shown in plain text so you can copy it."
      />
      <Select label="Role" value={role} onChange={(e) => setRole(e.target.value as Role)}>
        <option value="member">member</option>
        <option value="admin">admin</option>
        <option value="owner">owner</option>
      </Select>
    </Modal>
  );
}
