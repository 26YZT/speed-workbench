-- Durable per-turn token and cost ledger for agent work.
-- The owner and task references are intentionally denormalized: usage must
-- remain queryable after a team/task is deleted, while user_id keeps reads
-- isolated across accounts.
CREATE TABLE IF NOT EXISTS agent_usage (
    id              TEXT    PRIMARY KEY NOT NULL,
    user_id         TEXT    NOT NULL,
    task_id         TEXT,
    agent_id        TEXT,
    model           TEXT,
    input_tokens    INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens   INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    cost_est        REAL,
    conversation_id TEXT    NOT NULL,
    turn_id         TEXT    NOT NULL,
    created_at      INTEGER NOT NULL,
    UNIQUE (user_id, conversation_id, turn_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_usage_user_task_created
    ON agent_usage(user_id, task_id, created_at DESC, id DESC);
