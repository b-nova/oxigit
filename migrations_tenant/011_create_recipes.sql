CREATE TABLE IF NOT EXISTS recipes (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   session_id      TEXT NOT NULL,
   author_id       INTEGER NOT NULL,
   title           TEXT NOT NULL,
   description     TEXT NOT NULL DEFAULT '',
   ai_tool         TEXT NOT NULL,
   ai_model        TEXT,
   tags            TEXT,
   prompt_count    INTEGER NOT NULL DEFAULT 0,
   file_count      INTEGER NOT NULL DEFAULT 0,
   vibe_score      INTEGER,
   replay_count    INTEGER NOT NULL DEFAULT 0,
   is_public       BOOLEAN NOT NULL DEFAULT 1,
   created_at      TEXT NOT NULL DEFAULT (datetime('now')),
   UNIQUE(repo_id, session_id)
);

CREATE INDEX IF NOT EXISTS idx_recipes_author ON recipes(author_id);
CREATE INDEX IF NOT EXISTS idx_recipes_public ON recipes(is_public, created_at DESC);
