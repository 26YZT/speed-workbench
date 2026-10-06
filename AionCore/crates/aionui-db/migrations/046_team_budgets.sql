-- Optional per-team budget used by the product-factory cost guard.
CREATE TABLE IF NOT EXISTS team_budgets (
    team_id    TEXT PRIMARY KEY NOT NULL,
    user_id    TEXT NOT NULL,
    limit_usd  REAL NOT NULL CHECK (limit_usd >= 0),
    updated_at INTEGER NOT NULL,
    UNIQUE (user_id, team_id)
);

CREATE INDEX IF NOT EXISTS idx_team_budgets_user_id ON team_budgets(user_id);
