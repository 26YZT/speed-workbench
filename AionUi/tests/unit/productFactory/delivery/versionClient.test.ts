import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  activateProductVersion,
  getProductVersions,
  getSourceManifest,
  iterateProductVersion,
  snapshotProductVersion,
} from '@/renderer/pages/product-factory/components/Delivery/versionClient';
import {
  history,
  iterationRun,
  manifest,
  operation,
  product,
  sourceManifest,
  version,
  workingVersion,
} from './versionFixtures';

vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));
const response = (data: unknown) => new Response(JSON.stringify({ success: true, data }));

describe('code-only version API contracts', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });
  it('reads legacy unregistered runs without inventing a product or sending writes', async () => {
    vi.mocked(fetch).mockResolvedValue(
      response({ product: null, current_version_id: null, versions: [], operations: [] })
    );
    expect(await getProductVersions('run-1')).toEqual({
      product: undefined,
      currentVersionId: undefined,
      versions: [],
      operations: [],
    });
    expect(vi.mocked(fetch).mock.calls[0][1]?.method).toBeUndefined();
  });
  it('preserves version lineage and the selected code version', async () => {
    vi.mocked(fetch).mockResolvedValue(response({ ...history, versions: [version, workingVersion] }));
    const loaded = await getProductVersions('run-1');
    expect(loaded.product?.activeVersionId).toBe('version-1');
    expect(loaded.versions[1]).toMatchObject({ parentVersionId: 'version-1', parentRunId: 'run-1', state: 'working' });
  });
  it.each([
    { ...manifest, scope: 'everything' },
    { ...manifest, files: [{ path: '../database.sqlite', sha256: 'a'.repeat(64), size_bytes: 10 }] },
    { ...manifest, files: [{ path: 'main.py', sha256: 'short', size_bytes: 10 }] },
  ])('rejects unsupported scope, escaped paths and invalid hashes', async (invalidManifest) => {
    vi.mocked(fetch).mockResolvedValue(response({ ...sourceManifest, manifest: invalidManifest }));
    await expect(getSourceManifest('run-1')).rejects.toThrow('PRODUCT_FACTORY_INVALID_VERSION_RESPONSE');
  });
  it('sends only explicitly chosen paths, hashes, revision and the retained key', async () => {
    vi.mocked(fetch).mockResolvedValue(response({ product, version, operation }));
    const input = {
      expected_plan_revision: 1,
      idempotency_key: 'one-key',
      files: manifest.files.map(({ path, sha256 }) => ({ path, sha256 })),
    };
    await snapshotProductVersion('run-1', input);
    expect(JSON.parse(String(vi.mocked(fetch).mock.calls[0][1]?.body))).toEqual(input);
    expect(new Headers(vi.mocked(fetch).mock.calls[0][1]?.headers).get('x-csrf-token')).toBe('csrf');
  });
  it('creates a new draft with an independent budget without any model request', async () => {
    vi.mocked(fetch).mockResolvedValue(
      response({
        product,
        version: workingVersion,
        operation: { ...operation, kind: 'iterate', version_id: 'version-2' },
        run: iterationRun,
      })
    );
    const receipt = await iterateProductVersion('run-1', {
      source_version_id: 'version-1',
      expected_product_revision: 1,
      idempotency_key: 'iterate-key',
      change_request: 'Add export',
      budget_usd: 2,
    });
    expect(receipt.run).toMatchObject({ id: 'run-2', status: 'draft', budgetUsd: 2 });
    expect(vi.mocked(fetch)).toHaveBeenCalledTimes(1);
  });
  it('rejects a successful-looking activation response for an unsealed working version', async () => {
    vi.mocked(fetch).mockResolvedValue(
      response({ product, version: workingVersion, workspace_path: '/workspaces/run-2', scope: 'code_only' })
    );
    await expect(activateProductVersion('run-1', 'version-2', 1)).rejects.toThrow(
      'PRODUCT_FACTORY_INVALID_VERSION_RESPONSE'
    );
  });
});
