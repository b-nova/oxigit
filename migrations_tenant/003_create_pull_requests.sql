CREATE TABLE IF NOT EXISTS pull_requests (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   number          INTEGER NOT NULL,
   title           TEXT    NOT NULL,
   description     TEXT    NOT NULL DEFAULT '',
   author_id       INTEGER NOT NULL,
   source_branch   TEXT    NOT NULL,
   target_branch   TEXT    NOT NULL,
   status          TEXT    NOT NULL DEFAULT 'open',
   merged_by       INTEGER,
   created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
   updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
   UNIQUE(repo_id, number)
);

CREATE INDEX IF NOT EXISTS idx_pr_repo ON pull_requests(repo_id);
CREATE INDEX IF NOT EXISTS idx_pr_author ON pull_requests(author_id);
CREATE INDEX IF NOT EXISTS idx_pr_status ON pull_requests(repo_id, status);
