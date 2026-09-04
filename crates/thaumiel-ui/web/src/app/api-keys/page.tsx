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
import type { ApiKey, ApiKeyScope } from "@/lib/types";

export default function ApiKeysPage() {
  const { api, identity } = useAppState();
  const [keys, setKeys] = useState<ApiKey[] | null>(null);
  const [open, setOpen] = useState(false);
  const [revoking, setRevoking] = useState<string | null>(null);

  function refresh() {
    api.listApiKeys().then(setKeys);
  }

  useEffect(() => {
    if (!identity) return;
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [identity]);

  async function revoke(id: string) {
    setRevoking(id);
    try {
      await api.revokeApiKey(id);
      refresh();
    } finally {
      setRevoking(null);
    }
  }

  return (
    <DashboardShell>
      <PageHeader
        title="API Keys"
        subtitle="Machine credentials, used to call /v1/licenses/validate from a shipped application."
        action={
          <Button variant="primary" onClick={() => setOpen(true)}>
            <Plus size={15} /> New key
          </Button>
        }
      />

      <Card>
        <Table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Prefix</th>
              <th>Scope</th>
              <th>Created</th>
              <th>Last used</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {keys?.length === 0 && <EmptyState label="No API keys yet." />}
            {keys?.map((k) => (
              <tr key={k.id}>
                <td>{k.name}</td>
                <td className="mono">{k.key_prefix}</td>
                <td>
                  <Badge tone="accent" dot={false}>
                    {k.scope}
                  </Badge>
                </td>
                <td>{new Date(k.created_at).toLocaleDateString()}</td>
                <td>{k.last_used_at ? new Date(k.last_used_at).toLocaleDateString() : "Never"}</td>
                <td style={{ textAlign: "right" }}>
                  {!k.revoked_at && (
                    <Button size="sm" variant="danger" onClick={() => revoke(k.id)} disabled={revoking === k.id}>
                      {revoking === k.id ? "Revoking…" : "Revoke"}
                    </Button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </Table>
      </Card>

      {open && (
        <CreateApiKeyModal
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

function CreateApiKeyModal({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const { api } = useAppState();
  const [name, setName] = useState("");
  const [scope, setScope] = useState<ApiKeyScope>("validate_only");
  const [envTag, setEnvTag] = useState("live");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [plaintext, setPlaintext] = useState<string | null>(null);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      const created = await api.createApiKey(name.trim(), scope, envTag);
      setPlaintext(created.plaintext);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not create the key.");
    } finally {
      setBusy(false);
    }
  }

  if (plaintext) {
    return (
      <Modal title="Key created" onClose={onCreated} footer={<Button variant="primary" onClick={onCreated}>Done</Button>}>
        <p style={{ fontSize: 12, color: "var(--danger)" }}>
          Shown once. It isn&rsquo;t stored anywhere and can&rsquo;t be shown again.
        </p>
        <Input readOnly value={plaintext} onFocus={(e) => e.currentTarget.select()} className="mono" />
      </Modal>
    );
  }

  return (
    <Modal
      title="New API key"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} disabled={busy || !name.trim()}>
            {busy ? "Creating…" : "Create"}
          </Button>
        </>
      }
    >
      {error ? <p style={{ color: "var(--danger)", fontSize: 12 }}>{error}</p> : null}
      <Input label="Name" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="desktop client" />
      <Select label="Scope" value={scope} onChange={(e) => setScope(e.target.value as ApiKeyScope)}>
        <option value="validate_only">validate_only</option>
        <option value="license_manager">license_manager</option>
        <option value="admin">admin</option>
      </Select>
      <Select label="Environment tag" value={envTag} onChange={(e) => setEnvTag(e.target.value)}>
        <option value="live">live</option>
        <option value="test">test</option>
      </Select>
    </Modal>
  );
}
