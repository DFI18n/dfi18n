import Database from "better-sqlite3";

export interface CacheEntry {
  translation: string;
  model: string;
  promptVersion: string;
  createdAt: string;
}

export class TranslationCache {
  private readonly database: Database.Database;
  private readonly selectStatement: Database.Statement<[string], { translation: string }>;
  private readonly upsertStatement: Database.Statement<[string, string, string, string, string]>;

  constructor(path: string) {
    this.database = new Database(path);
    this.database.pragma("journal_mode = WAL");
    this.database.pragma("synchronous = NORMAL");
    this.database.exec(`
      CREATE TABLE IF NOT EXISTS translations (
        cache_key TEXT PRIMARY KEY,
        translation TEXT NOT NULL,
        model TEXT NOT NULL,
        prompt_version TEXT NOT NULL,
        created_at TEXT NOT NULL
      )
    `);
    this.selectStatement = this.database.prepare(
      "SELECT translation FROM translations WHERE cache_key = ?",
    );
    this.upsertStatement = this.database.prepare(`
      INSERT INTO translations (cache_key, translation, model, prompt_version, created_at)
      VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(cache_key) DO UPDATE SET
        translation = excluded.translation,
        model = excluded.model,
        prompt_version = excluded.prompt_version,
        created_at = excluded.created_at
    `);
  }

  get(key: string): string | undefined {
    return this.selectStatement.get(key)?.translation;
  }

  put(key: string, entry: CacheEntry): void {
    this.upsertStatement.run(key, entry.translation, entry.model, entry.promptVersion, entry.createdAt);
  }

  close(): void {
    this.database.close();
  }
}
