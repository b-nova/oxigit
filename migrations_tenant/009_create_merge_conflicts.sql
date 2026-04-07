CREATE TABLE IF NOT EXISTS merge_conflicts (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   user_id         INTEGER NOT NULL,
   operation_type  TEXT NOT NULL,
   target_ref      TEXT NOT NULL,
   source_ref      TEXT NOT NULL,
   merge_base      TEXT NOT NULL,
   auto_tree       TEXT,
   context_json    TEXT,
   status          TEXT NOT NULL DEFAULT 'pending',
   created_at      TEXT NOT NULL DEFAULT (datetime('now')),
   updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS merge_conflict_files (
   id                  INTEGER PRIMARY KEY AUTOINCREMENT,
   merge_conflict_id   INTEGER NOT NULL REFERENCES merge_conflicts(id) ON DELETE CASCADE,
   file_path           TEXT NOT NULL,
   conflict_type       TEXT NOT NULL,
   resolution          TEXT,
   resolved_content    TEXT,
   resolved_at         TEXT
);

CREATE INDEX IF NOT EXISTS idx_merge_conflicts_repo ON merge_conflicts(repo_id, user_id, status);
