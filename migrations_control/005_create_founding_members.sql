CREATE TABLE IF NOT EXISTS founding_members (
   id          INTEGER PRIMARY KEY AUTOINCREMENT,
   user_id     INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
   slot_number INTEGER NOT NULL,
   claimed_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
