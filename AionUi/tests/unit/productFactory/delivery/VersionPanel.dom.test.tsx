import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import VersionPanel from '@/renderer/pages/product-factory/components/Delivery/VersionPanel';
import {
  completedRun,
  history as initialHistory,
  iterationRun,
  operation,
  product,
  sourceManifest,
  version,
  workingVersion,
} from './versionFixtures';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { code?: string }) => (values?.code ? `${key}: ${values.code}` : key),
  }),
}));
vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));

let history: typeof initialHistory;
let calls: Array<{ path: string; method: string; body?: Record<string, unknown> }>;
let failSnapshot: boolean;
let loseSnapshotResponse: boolean;
let activationConflict: boolean;
let loseIterationResponse: boolean;
let source: typeof sourceManifest;
const opened = vi.fn();
const blocked = vi.fn();
const response = (data: unknown) => new Response(JSON.stringify({ success: true, data }));
const mount = () => render(<VersionPanel run={completedRun} onOpenRun={opened} onPreparationBlocked={blocked} />);
const emptyHistory = () =>
  ({ product: null, current_version_id: null, versions: [], operations: [] }) satisfies typeof initialHistory;

describe('explicit code versions and iteration', () => {
  beforeEach(() => {
    history = structuredClone(initialHistory);
    calls = [];
    failSnapshot = false;
    loseSnapshotResponse = false;
    activationConflict = false;
    loseIterationResponse = false;
    source = structuredClone(sourceManifest);
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: false,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }))
    );
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
    vi.stubGlobal(
      'fetch',
      vi.fn(async (path: string, init?: RequestInit) => {
        const method = init?.method ?? 'GET';
        const body = init?.body ? (JSON.parse(String(init.body)) as Record<string, unknown>) : undefined;
        calls.push({ path, method, body });
        if (path.endsWith('/versions') && method === 'GET') return response(history);
        if (path.endsWith('/source-manifest') && method === 'GET') return response(source);
        if (path.endsWith('/snapshot') && method === 'POST') {
          if (failSnapshot)
            return new Response(
              JSON.stringify({ error: 'changed', code: 'PRODUCT_FACTORY_VERSION_REVISION_CONFLICT' }),
              { status: 409 }
            );
          const receipt = {
            product,
            version,
            operation: { ...operation, idempotency_key: String(body?.idempotency_key) },
          };
          history = { ...initialHistory, operations: [receipt.operation] };
          if (loseSnapshotResponse) throw new Error('response lost');
          return response(receipt);
        }
        if (path.endsWith('/iterate') && method === 'POST') {
          const receipt = {
            product: { ...product, revision: 2 },
            version: workingVersion,
            operation: {
              ...operation,
              kind: 'iterate',
              version_id: 'version-2',
              idempotency_key: String(body?.idempotency_key),
            },
            run: iterationRun,
          };
          history = {
            ...initialHistory,
            product: receipt.product,
            versions: [version, workingVersion],
            operations: [receipt.operation],
          };
          if (loseIterationResponse) throw new Error('iteration response lost');
          return response(receipt);
        }
        if (path.endsWith('/activate') && method === 'POST') {
          if (activationConflict) {
            history.product = { ...product, revision: 2 };
            return new Response(
              JSON.stringify({ error: 'changed', code: 'PRODUCT_FACTORY_VERSION_REVISION_CONFLICT' }),
              { status: 409 }
            );
          }
          history.product = { ...product, revision: 2, active_version_id: 'version-1' };
          return response({
            product: history.product,
            version,
            scope: 'code_only',
            workspace_path: version.snapshot_path,
          });
        }
        throw new Error(`Unexpected ${method} ${path}`);
      })
    );
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('loads version history without creating snapshots, models or iterations', async () => {
    mount();
    await screen.findByText('productFactory.versions.states.sealed');
    expect(calls.every((call) => call.method === 'GET' && call.path.endsWith('/versions'))).toBe(true);
    expect(opened).not.toHaveBeenCalled();
  });

  it('asks for file review and permits a local snapshot without any model-price dependency', async () => {
    history = emptyHistory();
    mount();
    const review = await screen.findByRole('button', { name: 'productFactory.versions.reviewSource' });
    await waitFor(() => expect(review).not.toBeDisabled());
    fireEvent.click(review);
    expect(await screen.findByText('main.py')).toBeInTheDocument();
    expect(calls.some((call) => call.method === 'POST')).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.saveSnapshot' }));
    await screen.findByText('productFactory.versions.states.sealed');
    expect(calls.find((call) => call.path.endsWith('/snapshot'))?.body).toMatchObject({
      expected_plan_revision: 1,
      files: sourceManifest.manifest.files.map(({ path, sha256 }) => ({ path, sha256 })),
    });
  });

  it('requires the startup and acceptance guides in the explicit selection', async () => {
    history = emptyHistory();
    mount();
    const review = await screen.findByRole('button', { name: 'productFactory.versions.reviewSource' });
    await waitFor(() => expect(review).not.toBeDisabled());
    fireEvent.click(review);
    fireEvent.click(await screen.findByRole('checkbox', { name: 'START.md' }));
    expect(screen.getByRole('button', { name: 'productFactory.versions.saveSnapshot' })).toBeDisabled();
    expect(calls.some((call) => call.method === 'POST')).toBe(false);
  });

  it('creates a new draft with a separate budget and only opens it after the user submits', async () => {
    mount();
    const iterate = await screen.findByRole('button', { name: 'productFactory.versions.iterate' });
    await waitFor(() => expect(iterate).not.toBeDisabled());
    fireEvent.click(iterate);
    fireEvent.change(await screen.findByRole('textbox', { name: 'productFactory.versions.changeRequest' }), {
      target: { value: 'Add export' },
    });
    fireEvent.change(screen.getByRole('spinbutton', { name: 'productFactory.versions.iterationBudget' }), {
      target: { value: '2' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.createIteration' }));
    await waitFor(() => expect(opened).toHaveBeenCalledWith('run-2'));
    expect(calls.find((call) => call.path.endsWith('/iterate'))?.body).toMatchObject({
      source_version_id: 'version-1',
      expected_product_revision: 1,
      change_request: 'Add export',
      budget_usd: 2,
    });
    expect(calls.every((call) => call.path.includes('/versions'))).toBe(true);
  });

  it('blocks planning and activation for a copying version and stops polling after completion', async () => {
    history = { ...history, versions: [{ ...version, state: 'copying', snapshot_path: null, manifest: null }] };
    vi.useFakeTimers();
    await act(async () => {
      mount();
    });
    expect(blocked).toHaveBeenLastCalledWith(true);
    expect(screen.getByRole('button', { name: 'productFactory.versions.activate' })).toBeDisabled();
    history = initialHistory;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(blocked).toHaveBeenLastCalledWith(false);
    const stopped = calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });
    expect(calls).toHaveLength(stopped);
  });

  it('keeps the original key after conflict and requires a fresh review for a new request', async () => {
    history = emptyHistory();
    failSnapshot = true;
    mount();
    const review = await screen.findByRole('button', { name: 'productFactory.versions.reviewSource' });
    await waitFor(() => expect(review).not.toBeDisabled());
    fireEvent.click(review);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.versions.saveSnapshot' }));
    await screen.findByText('productFactory.versions.errorCode: PRODUCT_FACTORY_VERSION_REVISION_CONFLICT');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.close' }));
    const key = calls.find((call) => call.method === 'POST')?.body?.idempotency_key;
    failSnapshot = false;
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.retrySameRequest' }));
    await screen.findByText('productFactory.versions.states.sealed');
    expect(calls.filter((call) => call.method === 'POST').map((call) => call.body?.idempotency_key)).toEqual([
      key,
      key,
    ]);
  });

  it('recovers a lost snapshot response with a receipt read without repeating the write', async () => {
    history = emptyHistory();
    loseSnapshotResponse = true;
    mount();
    const review = await screen.findByRole('button', { name: 'productFactory.versions.reviewSource' });
    await waitFor(() => expect(review).not.toBeDisabled());
    fireEvent.click(review);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.versions.saveSnapshot' }));
    await screen.findByText('productFactory.versions.states.sealed');
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
  });

  it('closes a recovered iteration request instead of offering a duplicate creation', async () => {
    loseIterationResponse = true;
    mount();
    const iterate = await screen.findByRole('button', { name: 'productFactory.versions.iterate' });
    await waitFor(() => expect(iterate).not.toBeDisabled());
    fireEvent.click(iterate);
    fireEvent.change(await screen.findByRole('textbox', { name: 'productFactory.versions.changeRequest' }), {
      target: { value: 'Add export' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.createIteration' }));
    const openRun = await screen.findByRole('button', { name: 'productFactory.versions.openRun' });
    expect(screen.queryByRole('button', { name: 'productFactory.versions.createIteration' })).not.toBeInTheDocument();
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
    fireEvent.click(openRun);
    expect(opened).toHaveBeenCalledWith('run-2');
  });

  it('restores a retained copy request after reload without resending it', async () => {
    window.localStorage.setItem(
      'product-factory.version-write.run-1',
      JSON.stringify({
        kind: 'snapshot',
        input: {
          expected_plan_revision: 1,
          idempotency_key: 'retained-key',
          files: sourceManifest.manifest.files.map(({ path, sha256 }) => ({ path, sha256 })),
        },
      })
    );
    history = {
      ...history,
      versions: [{ ...version, state: 'copying', manifest: null, snapshot_path: null }],
      operations: [{ ...operation, state: 'copying', idempotency_key: 'retained-key' }],
    };
    mount();
    await screen.findByText('productFactory.versions.pendingHint');
    expect(screen.getByRole('button', { name: 'productFactory.versions.retrySameRequest' })).toBeDisabled();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('reads a fresh manifest and allocates a new key only after the user starts a changed request', async () => {
    history = emptyHistory();
    failSnapshot = true;
    mount();
    const review = await screen.findByRole('button', { name: 'productFactory.versions.reviewSource' });
    await waitFor(() => expect(review).not.toBeDisabled());
    fireEvent.click(review);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.versions.saveSnapshot' }));
    await screen.findByText('productFactory.versions.errorCode: PRODUCT_FACTORY_VERSION_REVISION_CONFLICT');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.close' }));
    source = {
      ...sourceManifest,
      plan_revision: 2,
      manifest: {
        ...sourceManifest.manifest,
        files: sourceManifest.manifest.files.map((file) => Object.assign({}, file, { sha256: 'b'.repeat(64) })),
      },
    };
    failSnapshot = false;
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.versions.newRequest' }));
    fireEvent.click(review);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.versions.saveSnapshot' }));
    await screen.findByText('productFactory.versions.states.sealed');
    const writes = calls.filter((call) => call.method === 'POST');
    expect(writes[1].body).toMatchObject({
      expected_plan_revision: 2,
      files: source.manifest.files.map(({ path, sha256 }) => ({ path, sha256 })),
    });
    expect(writes[1].body?.idempotency_key).not.toEqual(writes[0].body?.idempotency_key);
    expect(calls.filter((call) => call.path.endsWith('/source-manifest'))).toHaveLength(2);
  });

  it('uses product revision CAS for activation and never automatically retries a conflict', async () => {
    history.product = { ...product, active_version_id: null };
    activationConflict = true;
    mount();
    const activate = await screen.findByRole('button', { name: 'productFactory.versions.activate' });
    await waitFor(() => expect(activate).not.toBeDisabled());
    fireEvent.click(activate);
    await screen.findByText('productFactory.versions.errorCode: PRODUCT_FACTORY_VERSION_REVISION_CONFLICT');
    expect(calls.find((call) => call.path.endsWith('/activate'))?.body).toEqual({
      version_id: 'version-1',
      expected_product_revision: 1,
    });
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
  });

  it('updates the selected-code marker after a confirmed activation without launching the product', async () => {
    history.product = { ...product, active_version_id: null };
    mount();
    const activate = await screen.findByRole('button', { name: 'productFactory.versions.activate' });
    await waitFor(() => expect(activate).not.toBeDisabled());
    fireEvent.click(activate);
    expect(await screen.findByText('productFactory.versions.active')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'productFactory.versions.activate' })).toBeDisabled();
    expect(calls.every((call) => call.path.includes('/versions'))).toBe(true);
  });
});
