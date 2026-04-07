CREATE TABLE IF NOT EXISTS repository_index (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    org_slug        TEXT NOT NULL,
    owner_id        INTEGER NOT NULL,
    owner_username  TEXT NOT NULL,
    repo_name       TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    is_private      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(owner_username, repo_name)
);

CREATE INDEX IF NOT EXISTS idx_repo_index_org ON repository_index(org_slug);
CREATE INDEX IF NOT EXISTS idx_repo_index_owner ON repository_index(owner_username);
CREATE INDEX IF NOT EXISTS idx_repo_index_public ON repository_index(is_private, updated_at DESC);
