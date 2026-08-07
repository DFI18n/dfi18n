export interface ServiceConfig {
  clientHeaderValue: string;
  upstreamApiKey: string;
  upstreamBaseUrl: string;
  upstreamModel: string;
  promptVersion: string;
  maxTextLength: number;
  upstreamTimeoutMs: number;
  maxUpstreamConcurrency: number;
  upstreamExtraBody: Record<string, unknown>;
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): ServiceConfig {
  const upstreamApiKey = env.UPSTREAM_API_KEY?.trim();
  if (!upstreamApiKey) throw new Error("UPSTREAM_API_KEY is required");

  return {
    clientHeaderValue: env.CLIENT_HEADER_VALUE?.trim() || "dfi18n-runtime-v1",
    upstreamApiKey,
    upstreamBaseUrl: env.UPSTREAM_BASE_URL?.trim() || "https://api.deepseek.com",
    upstreamModel: env.UPSTREAM_MODEL?.trim() || "deepseek-v4-flash",
    promptVersion: env.PROMPT_VERSION?.trim() || "df-zh-hans-v2",
    maxTextLength: boundedInteger(env.MAX_TEXT_LENGTH, 4000, 1, 20_000),
    upstreamTimeoutMs: boundedInteger(env.UPSTREAM_TIMEOUT_MS, 30_000, 1_000, 120_000),
    maxUpstreamConcurrency: boundedInteger(env.MAX_UPSTREAM_CONCURRENCY, 8, 1, 64),
    upstreamExtraBody: parseObject(env.UPSTREAM_EXTRA_BODY_JSON),
  };
}

function boundedInteger(value: string | undefined, fallback: number, minimum: number, maximum: number): number {
  const parsed = Number.parseInt(value ?? "", 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(maximum, Math.max(minimum, parsed));
}

function parseObject(value: string | undefined): Record<string, unknown> {
  if (!value) return {};
  try {
    const parsed: unknown = JSON.parse(value);
    if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // Report one consistent configuration error below.
  }
  throw new Error("UPSTREAM_EXTRA_BODY_JSON must be a JSON object");
}
