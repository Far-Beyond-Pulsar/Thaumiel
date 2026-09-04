"use client";

import { useEffect, useMemo, useState } from "react";
import { Plus, ShieldCheck } from "lucide-react";
import { DashboardShell } from "@/components/DashboardShell";
import { PageHeader } from "@/components/PageHeader";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { Input, Select, Textarea } from "@/components/ui/Field";
import { Modal } from "@/components/ui/Modal";
import { StatusBadge } from "@/components/ui/Badge";
import { EmptyState, Table } from "@/components/ui/Table";
import { ApiClient, ApiError } from "@/lib/api";
import { useAppState } from "@/lib/app-state";
import type { KeygenBackendInfo, LicenseKey, Product, ValidateLicenseResponse } from "@/lib/types";

export default function LicensesPage() {
  const { api, identity } = useAppState();
  const [licenses, setLicenses] = useState<LicenseKey[] | null>(null);
  const [products, setProducts] = useState<Product[]>([]);
  const [backends, setBackends] = useState<KeygenBackendInfo[]>([]);
  const [generateOpen, setGenerateOpen] = useState(false);
  const [validateOpen, setValidateOpen] = useState(false);
  const [revoking, setRevoking] = useState<string | null>(null);

  const productName = useMemo(() => {
    const map = new Map(products.map((p) => [p.id, p.name]));
    return (id: string) => map.get(id) ?? id;
  }, [products]);

  function refresh() {
    api.listLicenses().then(setLicenses);
  }

  useEffect(() => {
    if (!identity) return;
    refresh();
    api.listProducts().then(setProducts);
    api.keygenBackends().then(setBackends);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [identity]);

  async function revoke(id: string) {
    setRevoking(id);
    try {
      await api.revokeLicense(id);
      refresh();
    } finally {
      setRevoking(null);
    }
  }

  return (
    <DashboardShell>
      <PageHeader
        title="Licenses"
        subtitle="Generate, inspect, and revoke license keys."
        action={
          <div style={{ display: "flex", gap: 8 }}>
            <Button onClick={() => setValidateOpen(true)}>
              <ShieldCheck size={15} /> Validate a key
            </Button>
            <Button variant="primary" onClick={() => setGenerateOpen(true)} disabled={products.length === 0}>
              <Plus size={15} /> Generate license
            </Button>
          </div>
        }
      />

      {products.length === 0 && (
        <p style={{ fontSize: 13, color: "var(--text-faint)", marginBottom: 16 }}>
          Create a product first — licenses always belong to one.
        </p>
      )}

      <Card>
        <Table>
          <thead>
            <tr>
              <th>Key</th>
              <th>Product</th>
              <th>Backend</th>
              <th>Status</th>
              <th>Seats</th>
              <th>Expires</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {licenses?.length === 0 && <EmptyState label="No licenses yet." />}
            {licenses?.map((l) => (
              <tr key={l.id}>
                <td className="mono" title={l.key}>
                  {l.key.length > 28 ? `${l.key.slice(0, 28)}…` : l.key}
                </td>
                <td>{productName(l.product_id)}</td>
                <td className="mono">{l.backend_id}</td>
                <td>
                  <StatusBadge status={l.status} />
                </td>
                <td>{l.seats}</td>
                <td>{l.expires_at ? new Date(l.expires_at).toLocaleDateString() : "Never"}</td>
                <td style={{ textAlign: "right" }}>
                  {l.status === "active" && (
                    <Button size="sm" variant="danger" onClick={() => revoke(l.id)} disabled={revoking === l.id}>
                      {revoking === l.id ? "Revoking…" : "Revoke"}
                    </Button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </Table>
      </Card>

      {generateOpen && (
        <GenerateLicenseModal
          products={products}
          backends={backends}
          onClose={() => setGenerateOpen(false)}
          onCreated={() => {
            setGenerateOpen(false);
            refresh();
          }}
        />
      )}

      {validateOpen && <ValidateLicenseModal products={products} onClose={() => setValidateOpen(false)} />}
    </DashboardShell>
  );
}

function GenerateLicenseModal({
  products,
  backends,
  onClose,
  onCreated,
}: {
  products: Product[];
  backends: KeygenBackendInfo[];
  onClose: () => void;
  onCreated: (license: LicenseKey) => void;
}) {
  const { api } = useAppState();
  const [productId, setProductId] = useState(products[0]?.id ?? "");
  const [seats, setSeats] = useState(1);
  const [backendId, setBackendId] = useState("");
  const [expiresAt, setExpiresAt] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<LicenseKey | null>(null);

  async function submit() {
    setError(null);
    setBusy(true);
    try {
      const license = await api.generateLicense({
        productId,
        seats,
        backendId: backendId || undefined,
        expiresAt: expiresAt ? new Date(expiresAt).toISOString() : undefined,
      });
      setResult(license);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not generate the license.");
    } finally {
      setBusy(false);
    }
  }

  if (result) {
    return (
      <Modal title="License generated" onClose={() => onCreated(result)} footer={<Button variant="primary" onClick={() => onCreated(result)}>Done</Button>}>
        <p style={{ fontSize: 12, color: "var(--text-dim)" }}>
          This key is also stored and retrievable later from the license list, unlike an API key.
        </p>
        <Textarea readOnly value={result.key} onFocus={(e) => e.currentTarget.select()} />
      </Modal>
    );
  }

  return (
    <Modal
      title="Generate license"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={submit} disabled={busy || !productId}>
            {busy ? "Generating…" : "Generate"}
          </Button>
        </>
      }
    >
      {error ? <p style={{ color: "var(--danger)", fontSize: 12 }}>{error}</p> : null}
      <Select label="Product" value={productId} onChange={(e) => setProductId(e.target.value)}>
        {products.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </Select>
      <Input label="Seats" type="number" min={1} value={seats} onChange={(e) => setSeats(Number(e.target.value) || 1)} />
      <Select label="Keygen backend" value={backendId} onChange={(e) => setBackendId(e.target.value)}>
        <option value="">Product default</option>
        {backends.map((b) => (
          <option key={b.id} value={b.id}>
            {b.id}
          </option>
        ))}
      </Select>
      <Input label="Expires at" type="date" value={expiresAt} onChange={(e) => setExpiresAt(e.target.value)} hint="Optional." />
    </Modal>
  );
}

function ValidateLicenseModal({ products, onClose }: { products: Product[]; onClose: () => void }) {
  const { apiBaseUrl } = useAppState();
  const [apiKey, setApiKey] = useState("");
  const [productId, setProductId] = useState(products[0]?.id ?? "");
  const [key, setKey] = useState("");
  const [fingerprint, setFingerprint] = useState("");
  const [result, setResult] = useState<ValidateLicenseResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit() {
    setError(null);
    setBusy(true);
    setResult(null);
    try {
      const client = new ApiClient(apiBaseUrl, apiKey.trim());
      const res = await client.validateLicense(key.trim(), productId, fingerprint.trim() || undefined);
      setResult(res);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not reach the API.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="Validate a license"
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button variant="primary" onClick={submit} disabled={busy || !apiKey || !key || !productId}>
            {busy ? "Checking…" : "Check"}
          </Button>
        </>
      }
    >
      <p style={{ fontSize: 12, color: "var(--text-dim)" }}>
        Uses a validate-scoped API key, the same call a shipped application would make — not your admin session.
      </p>
      {error ? <p style={{ color: "var(--danger)", fontSize: 12 }}>{error}</p> : null}
      <Input label="API key" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="thm_live_…" />
      <Select label="Product" value={productId} onChange={(e) => setProductId(e.target.value)}>
        {products.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </Select>
      <Input label="License key" value={key} onChange={(e) => setKey(e.target.value)} />
      <Input label="Machine fingerprint" value={fingerprint} onChange={(e) => setFingerprint(e.target.value)} hint="Optional." />

      {result && (
        <div
          style={{
            borderRadius: 8,
            padding: 12,
            fontSize: 13,
            background: result.valid ? "var(--success-dim)" : "var(--danger-dim)",
            color: result.valid ? "var(--success)" : "var(--danger)",
          }}
        >
          {result.valid ? "Valid" : `Invalid — ${result.reason}`}
          {result.seats_total != null && (
            <div style={{ color: "var(--text-dim)", marginTop: 4 }}>
              Seats: {result.seats_used}/{result.seats_total}
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
