CREATE TABLE IF NOT EXISTS issue_comments (
   id          INTEGER PRIMARY KEY AUTOINCREMENT,
   issue_id    INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
   author_id   INTEGER NOT NULL,
   body        TEXT    NOT NULL,
   created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_issue_comments_issue ON issue_comments(issue_id);
