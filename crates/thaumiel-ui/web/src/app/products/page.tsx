"use client";

import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { Modal } from "@/components/ui/Modal";
import { EmptyState, Table } from "@/components/ui/Table";
import { ApiError } from "@/lib/api";
import { useAppState } from "@/lib/app-state";
import type { KeygenBackendInfo, Product } from "@/lib/types";

export default function ProductsPage() {
  const { api, identity } = useAppState();
  const [products, setProducts] = useState<Product[] | null>(null);
  const [backends, setBackends] = useState<KeygenBackendInfo[]>([]);
  const [open, setOpen] = useState(false);

  function refresh() {
    api.listProducts().then(setProducts);
  }

  useEffect(() => {
    if (!identity) return;
    refresh();
    api.keygenBackends().then(setBackends);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [identity]);

  return (
    <DashboardShell>
      <PageHeader
        title="Products"
        subtitle="Each product picks a default license key format."
        action={
          <Button variant="primary" onClick={() => setOpen(true)}>
            <Plus size={15} /> New product
          </Button>
        }
      />

      <Card>
        <Table>
          <thead>
            <tr>
              <th>Name</th>
              <th>Default keygen backend</th>
              <th>Created</th>
              <th>Product ID</th>
            </tr>
          </thead>
          <tbody>
            {products?.length === 0 && <EmptyState label="No products yet." />}
            {products?.map((p) => (
              <tr key={p.id}>
                <td>{p.name}</td>
                <td className="mono">{p.default_keygen_backend}</td>
                <td>{new Date(p.created_at).toLocaleDateString()}</td>
                <td className="mono" style={{ color: "var(--text-faint)" }}>
                  {p.id}
                </td>
              </tr>
            ))}
          </tbody>
        </Table>
      </Card>

      {open && (
        <CreateProductModal
          backends={backends}
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

function CreateProductModal({
  backends,
  onClose,
  onCreated,
}: {
  backends: KeygenBackendInfo[];
  onClose: () => void;
  onCreated: () => void;
}) {
  const { api } = useAppState();
  const [name, setName] = useState("");
  const [backend, setBackend] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      await api.createProduct(name.trim(), backend || undefined);
      onCreated();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not create the product.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="New product"
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
      <Input label="Name" autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="Widget Pro" />
      <Select label="Default keygen backend" value={backend} onChange={(e) => setBackend(e.target.value)}>
        <option value="">Server default</option>
        {backends.map((b) => (
          <option key={b.id} value={b.id}>
            {b.id} {b.offline_verifiable ? "(offline-verifiable)" : ""}
          </option>
        ))}
      </Select>
    </Modal>
  );
}
