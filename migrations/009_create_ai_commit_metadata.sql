CREATE TABLE IF NOT EXISTS ai_commit_metadata (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_sha      TEXT    NOT NULL,
    ai_tool         TEXT    NOT NULL,
    ai_model        TEXT,
    ai_prompt       TEXT,
    ai_session_id   TEXT,
    ai_files_touched TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, commit_sha)
);

CREATE INDEX IF NOT EXISTS idx_ai_metadata_repo ON ai_commit_metadata(repo_id);
CREATE INDEX IF NOT EXISTS idx_ai_metadata_session ON ai_commit_metadata(repo_id, ai_session_id);
