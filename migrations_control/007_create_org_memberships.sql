CREATE TABLE IF NOT EXISTS org_memberships (
   id         INTEGER PRIMARY KEY AUTOINCREMENT,
   org_id     INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
   user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
   role       TEXT NOT NULL DEFAULT 'member',
   created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_org_memberships_org_user ON org_memberships(org_id, user_id);
CREATE INDEX IF NOT EXISTS idx_org_memberships_user_id ON org_memberships(user_id);
