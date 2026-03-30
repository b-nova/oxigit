CREATE TABLE IF NOT EXISTS ai_diff_summaries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id       INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha    TEXT    NOT NULL,
    summary       TEXT    NOT NULL,
    risk_flags    TEXT,
    generated_by  TEXT    NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);

CREATE INDEX IF NOT EXISTS idx_ai_diff_summaries_repo ON ai_diff_summaries(repo_id);
