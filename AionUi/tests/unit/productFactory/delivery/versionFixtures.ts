import { apiRun, run } from '../planning/fixtures';
export const completedRun = { ...run, status: 'completed' as const };
export const product = {
  id: 'product-1',
  name: 'Product',
  active_version_id: 'version-1' as string | null,
  revision: 1,
  created_at: 1,
  updated_at: 1,
};
export const manifest = {
  version: 1,
  scope: 'code_only',
  files: ['START.md', 'ACCEPTANCE.md', 'main.py'].map((path) => ({ path, sha256: 'a'.repeat(64), size_bytes: 10 })),
  total_bytes: 30,
};
export const sourceManifest = {
  run_id: 'run-1',
  plan_revision: 1,
  manifest,
  excluded_count: 2,
  warnings: ['RUNTIME_DATA_NOT_INCLUDED'],
};
export const version = {
  id: 'version-1',
  product_id: 'product-1',
  run_id: 'run-1',
  parent_run_id: null as string | null,
  parent_version_id: null as string | null,
  version_no: 1,
  state: 'sealed',
  snapshot_path: '/snapshots/version-1' as string | null,
  manifest: manifest as typeof manifest | null,
  change_request: null as string | null,
  error_code: null as string | null,
  created_at: 1,
  updated_at: 1,
  sealed_at: 1 as number | null,
};
export const operation = {
  id: 'operation-1',
  kind: 'snapshot',
  state: 'complete',
  version_id: 'version-1',
  idempotency_key: 'key-1',
  error_code: null as string | null,
  created_at: 1,
  updated_at: 1,
};
export const history = {
  product: product as typeof product | null,
  current_version_id: 'version-1' as string | null,
  versions: [version],
  operations: [] as (typeof operation)[],
};
export const iterationRun = {
  ...apiRun,
  id: 'run-2',
  status: 'draft',
  budget_usd: 2,
  workspace_path: '/workspaces/run-2',
};
export const workingVersion = {
  ...version,
  id: 'version-2',
  run_id: 'run-2',
  parent_run_id: 'run-1',
  parent_version_id: 'version-1',
  version_no: 2,
  state: 'working',
  snapshot_path: null,
  manifest: null,
  sealed_at: null,
};
