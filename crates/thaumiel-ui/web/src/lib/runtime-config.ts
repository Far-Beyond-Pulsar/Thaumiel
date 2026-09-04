// The static export has no idea, at build time, what API this dashboard
// should talk to -- that's decided per-deployment by whoever runs the
// thaumiel-ui binary (see crates/thaumiel-ui/config/default.toml). At
// runtime the binary serves a small, non-cached /thaumiel-ui-config.json
// generated from its own loaded config; we fetch that once on boot and fall
// back to a known-good default if it's missing (e.g. this bundle is being
// served some other way, like `next dev`, or a plain static file host).

export const DEFAULT_API_BASE_URL = "http://localhost:8080";

export interface RuntimeConfig {
  apiBaseUrl: string;
}

export async function loadRuntimeConfig(): Promise<RuntimeConfig> {
  try {
    const res = await fetch("/thaumiel-ui-config.json", { cache: "no-store" });
    if (!res.ok) return { apiBaseUrl: DEFAULT_API_BASE_URL };
    const data = (await res.json()) as Partial<RuntimeConfig>;
    return { apiBaseUrl: data.apiBaseUrl || DEFAULT_API_BASE_URL };
  } catch {
    return { apiBaseUrl: DEFAULT_API_BASE_URL };
  }
}
