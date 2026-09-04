// Mirrors the JSON shapes returned by thaumiel-server -- see docs/API.md and
// crates/thaumiel-core/src/models.rs in the workspace root.

export type Role = "owner" | "admin" | "member";

export interface Identity {
  user_id: string;
  org_id: string;
  email: string;
  role: Role;
}

export interface User {
  id: string;
  org_id: string;
  email: string;
  role: Role;
  created_at: string;
}

export interface SessionResponse {
  token: string;
  identity: Identity;
}

export interface Organization {
  id: string;
  name: string;
  created_at: string;
}

export interface Product {
  id: string;
  org_id: string;
  name: string;
  default_keygen_backend: string;
  created_at: string;
}

export type LicenseStatus = "active" | "suspended" | "revoked" | "expired";

export interface LicenseKey {
  id: string;
  org_id: string;
  product_id: string;
  backend_id: string;
  key: string;
  status: LicenseStatus;
  seats: number;
  expires_at: string | null;
  metadata: Record<string, string>;
  created_at: string;
  revoked_at: string | null;
}

export type ApiKeyScope = "admin" | "license_manager" | "validate_only";

export interface ApiKey {
  id: string;
  org_id: string;
  name: string;
  key_hash: string;
  key_prefix: string;
  scope: ApiKeyScope;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface CreateApiKeyResponse extends ApiKey {
  plaintext: string;
}

export interface AuditLogEntry {
  id: string;
  org_id: string;
  actor: string;
  action: string;
  target: string;
  metadata: Record<string, string>;
  created_at: string;
}

export interface KeygenBackendInfo {
  id: string;
  description: string;
  offline_verifiable: boolean;
}

export interface UsageDayCount {
  date: string;
  count: number;
}

export interface UsageSummary {
  products: number;
  licenses_total: number;
  licenses_active: number;
  api_keys_active: number;
  counts_capped_at: number;
  validate_calls_last_14_days: UsageDayCount[];
}

export interface ValidateLicenseResponse {
  valid: boolean;
  reason: string | null;
  seats_total: number | null;
  seats_used: number | null;
}

export interface ApiErrorBody {
  error: { category: string; message: string };
}
