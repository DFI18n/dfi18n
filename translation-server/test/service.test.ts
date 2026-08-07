import { afterEach, describe, expect, it, vi } from "vitest";

import { TranslationCache } from "../src/cache.js";
import type { ServiceConfig } from "../src/config.js";
import { cacheKey, createApp, normalizeText } from "../src/service.js";

const caches: TranslationCache[] = [];
const makeCache = (): TranslationCache => {
  const cache = new TranslationCache(":memory:");
  caches.push(cache);
  return cache;
};

const config = (): ServiceConfig => ({
  clientHeaderValue: "dfi18n-runtime-v1",
  upstreamApiKey: "upstream-secret",
  upstreamBaseUrl: "https://llm.example/v1",
  upstreamModel: "example-model",
  promptVersion: "test-v1",
  maxTextLength: 100,
  upstreamTimeoutMs: 5000,
  maxUpstreamConcurrency: 4,
  upstreamExtraBody: {},
});

afterEach(() => {
  vi.restoreAllMocks();
  while (caches.length) caches.pop()?.close();
});

describe("translation server", () => {
  it("normalizes text and generates stable cache keys", () => {
    expect(normalizeText("  He is   angry.  ")).toBe("He is angry.");
    expect(cacheKey("A.", "v1", "m")).toBe(cacheKey("A.", "v1", "m"));
  });

  it("hides the endpoint when the static client header is missing", async () => {
    const app = createApp(config(), makeCache());
    const response = await app.inject({ method: "POST", url: "/v1/translate", payload: { text: "Hello." } });
    expect(response.statusCode).toBe(404);
    await app.close();
  });

  it("calls the upstream once and returns the cached result next time", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(new Response(JSON.stringify({
      choices: [{ message: { content: "他很容易发怒。" } }],
    }), { status: 200, headers: { "content-type": "application/json" } }));
    const app = createApp(config(), makeCache());
    const headers = { "x-dfi18n-client": "dfi18n-runtime-v1" };

    const first = await app.inject({
      method: "POST", url: "/v1/translate", headers,
      payload: { text: "  He is quick   to anger.  " },
    });
    expect(first.json()).toEqual({ translation: "他很容易发怒。", cached: false });

    const second = await app.inject({
      method: "POST", url: "/v1/translate", headers,
      payload: { text: "He is quick to anger." },
    });
    expect(second.json()).toEqual({ translation: "他很容易发怒。", cached: true });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await app.close();
  });

  it("coalesces concurrent requests for the same sentence", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => {
      await new Promise<void>((resolve) => setTimeout(resolve, 25));
      return new Response(JSON.stringify({ choices: [{ message: { content: "翻译结果。" } }] }), { status: 200 });
    });
    const app = createApp(config(), makeCache());
    const request = {
      method: "POST" as const,
      url: "/v1/translate",
      headers: { "x-dfi18n-client": "dfi18n-runtime-v1" },
      payload: { text: "The same sentence." },
    };

    const responses = await Promise.all([app.inject(request), app.inject(request)]);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(responses[0].json()).toEqual({ translation: "翻译结果。", cached: false });
    expect(responses[1].json()).toEqual({ translation: "翻译结果。", cached: false });
    await app.close();
  });
});
