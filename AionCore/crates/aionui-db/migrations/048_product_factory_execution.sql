-- One explicit first-task dispatch attempt per factory run. A mailbox ack is
-- not evidence that an Agent turn started or that a product was delivered.
CREATE TABLE IF NOT EXISTS product_factory_execution (
    run_id       TEXT PRIMARY KEY NOT NULL REFERENCES product_factory_runs(id) ON DELETE CASCADE,
    user_id      TEXT NOT NULL,
    team_id      TEXT NOT NULL,
    task_id      TEXT NOT NULL,
    state        TEXT NOT NULL CHECK (state IN ('pending', 'enqueued', 'uncertain')),
    message_id   TEXT,
    team_run_id  TEXT,
    requested_at INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    CHECK (state != 'enqueued' OR (length(message_id) > 0 AND length(team_run_id) > 0))
);
