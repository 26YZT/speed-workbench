ALTER TABLE product_factory_runs ADD COLUMN plan_revision INTEGER NOT NULL DEFAULT 1 CHECK(plan_revision >= 1);
CREATE TABLE IF NOT EXISTS product_factory_planning (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    run_id TEXT NOT NULL REFERENCES product_factory_runs(id) ON DELETE CASCADE,
    phase TEXT NOT NULL CHECK(phase IN ('interview','blueprint','task_graph')),
    input_plan_revision INTEGER NOT NULL CHECK(input_plan_revision >= 1),
    input_hash TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    assistant_id TEXT NOT NULL,
    model TEXT NOT NULL,
    assistant_snapshot_json TEXT,
    conversation_id TEXT,
    app_turn_id TEXT,
    state TEXT NOT NULL CHECK(state IN ('reserved','preparing','running','candidate_ready','applied','failed','cancelled','uncertain')),
    result_json TEXT,
    error_code TEXT,
    started_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(user_id,run_id,idempotency_key)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_factory_planning_live_revision
ON product_factory_planning(user_id,run_id,phase,input_plan_revision)
WHERE state IN ('reserved','preparing','running','candidate_ready','uncertain');
CREATE INDEX IF NOT EXISTS idx_factory_planning_owner_run ON product_factory_planning(user_id,run_id,created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_factory_planning_conversation ON product_factory_planning(conversation_id) WHERE conversation_id IS NOT NULL;
