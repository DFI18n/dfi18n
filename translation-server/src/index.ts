import { mkdirSync } from "node:fs";
import { dirname } from "node:path";

import { TranslationCache } from "./cache.js";
import { loadConfig } from "./config.js";
import { createApp } from "./service.js";

const port = Number.parseInt(process.env.PORT ?? "3000", 10);
const host = process.env.HOST?.trim() || "0.0.0.0";
const databasePath = process.env.DATABASE_PATH?.trim() || "/data/translations.sqlite3";

mkdirSync(dirname(databasePath), { recursive: true });
const cache = new TranslationCache(databasePath);
const app = createApp(loadConfig(), cache);

for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.on(signal, async () => {
    await app.close();
    cache.close();
    process.exit(0);
  });
}

await app.listen({ host, port });
