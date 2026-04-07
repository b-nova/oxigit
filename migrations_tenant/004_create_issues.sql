CREATE TABLE IF NOT EXISTS issues (
   id          INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id     INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   number      INTEGER NOT NULL,
   title       TEXT    NOT NULL,
   description TEXT    NOT NULL DEFAULT '',
   author_id   INTEGER NOT NULL,
   status      TEXT    NOT NULL DEFAULT 'open',
   created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
   updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
   UNIQUE(repo_id, number)
);

CREATE INDEX IF NOT EXISTS idx_issues_repo ON issues(repo_id);
CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(repo_id, status);
