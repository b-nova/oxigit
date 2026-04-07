CREATE TABLE IF NOT EXISTS users (
   id          INTEGER PRIMARY KEY AUTOINCREMENT,
   username    TEXT    NOT NULL UNIQUE,
   email       TEXT    NOT NULL UNIQUE,
   password_hash TEXT  NOT NULL,
   is_admin    INTEGER NOT NULL DEFAULT 0,
   is_disabled INTEGER NOT NULL DEFAULT 0,
   created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
   updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email ON users(email);
