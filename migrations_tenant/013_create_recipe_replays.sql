CREATE TABLE IF NOT EXISTS recipe_replays (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
   user_id         INTEGER NOT NULL,
   target_repo_id  INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   target_branch   TEXT NOT NULL,
   mode            TEXT NOT NULL,
   status          TEXT NOT NULL DEFAULT 'pending',
   steps_applied   INTEGER NOT NULL DEFAULT 0,
   error_message   TEXT,
   created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_replays_recipe ON recipe_replays(recipe_id);
CREATE INDEX IF NOT EXISTS idx_replays_user ON recipe_replays(user_id);
