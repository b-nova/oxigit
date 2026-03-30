CREATE TABLE IF NOT EXISTS collaborators (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission  TEXT    NOT NULL DEFAULT 'write',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_collaborators_repo ON collaborators(repo_id);
CREATE INDEX IF NOT EXISTS idx_collaborators_user ON collaborators(user_id);
