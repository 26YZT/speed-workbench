-- Durable, owner-scoped state for the Product Factory workflow.
-- Team execution remains in the existing team tables; team_id only records the
-- idempotent handoff result so retries cannot create a second team.
CREATE TABLE IF NOT EXISTS product_factory_runs (
    id              TEXT    PRIMARY KEY NOT NULL,
    user_id         TEXT    NOT NULL,
    name            TEXT    NOT NULL,
    idea            TEXT    NOT NULL,
    target_user     TEXT    NOT NULL DEFAULT '',
    problem         TEXT    NOT NULL DEFAULT '',
    expected_output TEXT    NOT NULL DEFAULT '',
    workspace_path  TEXT    NOT NULL DEFAULT '',
    budget_usd      REAL    CHECK (budget_usd IS NULL OR budget_usd > 0),
    status          TEXT    NOT NULL DEFAULT 'draft' CHECK (status IN (
        'draft',
        'interviewing',
        'blueprint_generating',
        'blueprint_ready',
        'task_draft_generating',
        'task_draft_ready',
        'handed_off',
        'running',
        'in_review',
        'completed',
        'failed'
    )),
    interview_json  TEXT,
    blueprint_json  TEXT,
    task_draft_json TEXT,
    team_id         TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_product_factory_runs_user_updated
    ON product_factory_runs(user_id, updated_at DESC, id DESC);
