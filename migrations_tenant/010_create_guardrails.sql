CREATE TABLE IF NOT EXISTS guardrail_rules (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   category        TEXT NOT NULL,
   action          TEXT NOT NULL DEFAULT 'off',
   created_at      TEXT NOT NULL DEFAULT (datetime('now')),
   updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
   UNIQUE(repo_id, category)
);

CREATE TABLE IF NOT EXISTS guardrail_config (
   id                  INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id             INTEGER NOT NULL UNIQUE REFERENCES repositories(id) ON DELETE CASCADE,
   min_vibe_score      INTEGER,
   max_files_per_push  INTEGER,
   created_at          TEXT NOT NULL DEFAULT (datetime('now')),
   updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS guardrail_violations (
   id              INTEGER PRIMARY KEY AUTOINCREMENT,
   repo_id         INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
   commit_sha      TEXT NOT NULL,
   ref_name        TEXT,
   rule_category   TEXT NOT NULL,
   action_taken    TEXT NOT NULL,
   severity        TEXT NOT NULL DEFAULT 'medium',
   message         TEXT NOT NULL,
   file_path       TEXT,
   pushed_by       TEXT,
   created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_guardrail_rules_repo ON guardrail_rules(repo_id);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_repo ON guardrail_violations(repo_id);
CREATE INDEX IF NOT EXISTS idx_guardrail_violations_sha ON guardrail_violations(repo_id, commit_sha);
