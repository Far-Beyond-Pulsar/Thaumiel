import type {
  ApiErrorBody,
  ApiKey,
  ApiKeyScope,
  AuditLogEntry,
  CreateApiKeyResponse,
  KeygenBackendInfo,
  LicenseKey,
  Organization,
  Product,
  Role,
  SessionResponse,
  UsageSummary,
  User,
  ValidateLicenseResponse,
} from "./types";

export class ApiError extends Error {
  category: string;
  status: number;

  constructor(status: number, category: string, message: string) {
    super(message);
    this.status = status;
    this.category = category;
  }
}

export class ApiClient {
  constructor(private baseUrl: string, private token: string | null) {}

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: {
        "content-type": "application/json",
        ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });

    if (!res.ok) {
      let category = "internal";
      let message = res.statusText;
      try {
        const parsed = (await res.json()) as ApiErrorBody;
        category = parsed.error?.category ?? category;
        message = parsed.error?.message ?? message;
      } catch {
        // response wasn't JSON (e.g. the API is unreachable) -- keep the fallback message
      }
      throw new ApiError(res.status, category, message);
    }

    if (res.status === 204) return undefined as T;
    return (await res.json()) as T;
  }

  register(orgName: string, email: string, password: string) {
    return this.request<SessionResponse>("POST", "/v1/auth/register", { org_name: orgName, email, password });
  }

  login(orgId: string, email: string, password: string) {
    return this.request<SessionResponse>("POST", "/v1/auth/login", { org_id: orgId, email, password });
  }

  me() {
    return this.request<Organization>("GET", "/v1/organizations/me");
  }

  listUsers() {
    return this.request<User[]>("GET", "/v1/users");
  }

  createUser(email: string, password: string, role: Role) {
    return this.request<User>("POST", "/v1/users", { email, password, role });
  }

  listProducts() {
    return this.request<Product[]>("GET", "/v1/products");
  }

  createProduct(name: string, defaultKeygenBackend?: string) {
    return this.request<Product>("POST", "/v1/products", {
      name,
      default_keygen_backend: defaultKeygenBackend || undefined,
    });
  }

  getProduct(id: string) {
    return this.request<Product>("GET", `/v1/products/${id}`);
  }

  listLicenses() {
    return this.request<LicenseKey[]>("GET", "/v1/licenses");
  }

  getLicense(id: string) {
    return this.request<LicenseKey>("GET", `/v1/licenses/${id}`);
  }

  generateLicense(input: {
    productId: string;
    seats: number;
    expiresAt?: string;
    backendId?: string;
    metadata?: Record<string, string>;
  }) {
    return this.request<LicenseKey>("POST", "/v1/licenses/generate", {
      product_id: input.productId,
      seats: input.seats,
      expires_at: input.expiresAt || undefined,
      backend_id: input.backendId || undefined,
      metadata: input.metadata || {},
    });
  }

  revokeLicense(id: string) {
    return this.request<LicenseKey>("POST", `/v1/licenses/${id}/revoke`);
  }

  validateLicense(key: string, productId: string, machineFingerprint?: string) {
    return this.request<ValidateLicenseResponse>("POST", "/v1/licenses/validate", {
      key,
      product_id: productId,
      machine_fingerprint: machineFingerprint || undefined,
    });
  }

  listApiKeys() {
    return this.request<ApiKey[]>("GET", "/v1/api-keys");
  }

  createApiKey(name: string, scope: ApiKeyScope, envTag: string) {
    return this.request<CreateApiKeyResponse>("POST", "/v1/api-keys", { name, scope, env_tag: envTag });
  }

  revokeApiKey(id: string) {
    return this.request<ApiKey>("POST", `/v1/api-keys/${id}/revoke`);
  }

  listAuditLog() {
    return this.request<AuditLogEntry[]>("GET", "/v1/audit-log");
  }

  keygenBackends() {
    return this.request<KeygenBackendInfo[]>("GET", "/v1/keygen-backends");
  }

  usage() {
    return this.request<UsageSummary>("GET", "/v1/usage");
  }
}
