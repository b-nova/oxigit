CREATE TABLE IF NOT EXISTS recipe_steps (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   recipe_id       INTEGER NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
   step_order      INTEGER NOT NULL,
   prompt_text     TEXT,
   prompt_index    INTEGER,
   commit_message  TEXT NOT NULL,
   files_json      TEXT,
   diff_text       TEXT,
   created_at      TEXT NOT NULL DEFAULT (datetime('now')),
   UNIQUE(recipe_id, step_order)
);

CREATE INDEX IF NOT EXISTS idx_recipe_steps_recipe ON recipe_steps(recipe_id);
