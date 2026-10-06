-- Version lineage preserves old runs, execution receipts and usage independently.
CREATE TABLE IF NOT EXISTS product_factory_products (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    active_version_id TEXT,
    revision INTEGER NOT NULL CHECK(revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_factory_products_owner ON product_factory_products(user_id,id);

CREATE TABLE IF NOT EXISTS product_factory_versions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    product_id TEXT NOT NULL REFERENCES product_factory_products(id) ON DELETE RESTRICT,
    run_id TEXT NOT NULL REFERENCES product_factory_runs(id) ON DELETE RESTRICT,
    parent_run_id TEXT REFERENCES product_factory_runs(id) ON DELETE RESTRICT,
    parent_version_id TEXT REFERENCES product_factory_versions(id) ON DELETE RESTRICT,
    version_no INTEGER NOT NULL CHECK(version_no >= 1),
    state TEXT NOT NULL CHECK(state IN ('copying','working','sealed','failed')),
    snapshot_path TEXT,
    manifest_json TEXT,
    change_request TEXT,
    error_code TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    sealed_at INTEGER,
    UNIQUE(user_id,run_id),
    UNIQUE(product_id,version_no)
);
CREATE INDEX IF NOT EXISTS idx_factory_versions_owner_product ON product_factory_versions(user_id,product_id,version_no);

CREATE TABLE IF NOT EXISTS product_factory_version_operations (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    source_run_id TEXT NOT NULL REFERENCES product_factory_runs(id) ON DELETE RESTRICT,
    version_id TEXT NOT NULL REFERENCES product_factory_versions(id) ON DELETE RESTRICT,
    idempotency_key TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    input_plan_revision INTEGER,
    task_ids_json TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('snapshot','iterate')),
    state TEXT NOT NULL CHECK(state IN ('reserved','copying','complete','failed')),
    error_code TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(user_id,source_run_id,idempotency_key)
);
CREATE INDEX IF NOT EXISTS idx_factory_version_operations_owner_run ON product_factory_version_operations(user_id,source_run_id,created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_factory_version_live_copy ON product_factory_version_operations(user_id,version_id) WHERE state IN ('reserved','copying');
