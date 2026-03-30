CREATE TABLE IF NOT EXISTS user_settings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    llm_provider  TEXT,
    llm_api_key   TEXT,
    llm_model     TEXT,
    llm_base_url  TEXT,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
