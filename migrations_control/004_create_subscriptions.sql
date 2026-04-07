CREATE TABLE IF NOT EXISTS subscriptions (
   id                     INTEGER PRIMARY KEY AUTOINCREMENT,
   user_id                INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
   stripe_customer_id     TEXT NOT NULL,
   stripe_subscription_id TEXT,
   plan                   TEXT NOT NULL DEFAULT 'free',
   status                 TEXT NOT NULL DEFAULT 'active',
   current_period_end     TEXT,
   seats                  INTEGER NOT NULL DEFAULT 1,
   created_at             TEXT NOT NULL DEFAULT (datetime('now')),
   updated_at             TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_stripe_customer_id ON subscriptions(stripe_customer_id);
CREATE INDEX IF NOT EXISTS idx_subscriptions_stripe_subscription_id ON subscriptions(stripe_subscription_id);
