import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ExecutionStart from '@/renderer/pages/product-factory/components/ExecutionStart';
import type { ProductFactoryRun } from '@/renderer/pages/product-factory/types';

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'test-csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));

const run: ProductFactoryRun = {
  id: 'run-1',
  name: 'Product',
  idea: 'Build a product',
  targetUser: '',
  problem: '',
  expectedOutput: '',
  workspacePath: '/workspace/product',
  status: 'handed_off',
  teamId: 'team-1',
  createdAt: 0,
  updatedAt: 0,
  taskDraft: { version: 1, generatedBy: 'draft', confirmed: true, revision: 2, updatedAt: 0, tasks: [] },
};
const receipt = {
  state: 'enqueued',
  team_id: 'team-1',
  task_id: 'team-1-data-1',
  message_id: 'mail-1',
  team_run_id: 'turn-1',
  requested_at: 1,
  updated_at: 2,
};
let execution:
  | (Omit<typeof receipt, 'message_id' | 'team_run_id'> & { message_id: string | null; team_run_id: string | null })
  | null;
let startFailure: boolean;
let startCode: string | undefined;
let readFailure: boolean;
let calls: Array<{ path: string; method: string; body?: unknown; csrf: string | null }>;
const response = (data: unknown) => new Response(JSON.stringify({ data }), { status: 200 });

describe('explicit first-task execution', () => {
  beforeEach(() => {
    execution = null;
    startFailure = false;
    startCode = undefined;
    readFailure = false;
    calls = [];
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
    vi.stubGlobal(
      'fetch',
      vi.fn(async (path: string, init?: RequestInit) => {
        const method = init?.method ?? 'GET';
        calls.push({
          path,
          method,
          body: init?.body ? JSON.parse(String(init.body)) : undefined,
          csrf: new Headers(init?.headers).get('x-csrf-token'),
        });
        if (path.endsWith('/execution')) {
          if (readFailure) throw new Error('offline');
          return response({ execution });
        }
        if (path.endsWith('/start')) {
          if (startCode) {
            return new Response(JSON.stringify({ code: startCode, error: 'Team cost cannot be verified' }), {
              status: 409,
            });
          }
          if (startFailure) {
            execution = { ...receipt, state: 'pending', message_id: null, team_run_id: null };
            return new Response(JSON.stringify({ code: 'INTERNAL', error: 'response lost' }), { status: 500 });
          }
          execution = receipt;
          return response({ execution });
        }
        throw new Error(`Unexpected request ${path}`);
      })
    );
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it('loading a handed-off run never starts an Agent', async () => {
    render(<ExecutionStart run={run} />);
    await screen.findByRole('button', { name: 'productFactory.execution.start' });
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(0);
  });

  it('requires explicit confirmation and posts the confirmed revision with CSRF', async () => {
    render(<ExecutionStart run={run} />);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.start' }));
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(0);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.confirm' }));
    await screen.findByText('productFactory.execution.enqueued');
    const posted = calls.filter((call) => call.method === 'POST');
    expect(posted).toEqual([
      {
        path: '/api/product-factory/runs/run-1/start',
        method: 'POST',
        body: { expected_revision: 2 },
        csrf: 'test-csrf',
      },
    ]);
    expect(screen.queryByRole('button', { name: 'productFactory.execution.start' })).not.toBeInTheDocument();
  });

  it('reload reads the durable receipt and offers no duplicate start', async () => {
    execution = receipt;
    render(<ExecutionStart run={run} />);
    await screen.findByText('productFactory.execution.enqueued');
    expect(screen.queryByRole('button', { name: 'productFactory.execution.start' })).not.toBeInTheDocument();
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(0);
  });

  it('a lost start response re-reads the reservation rather than resending', async () => {
    startFailure = true;
    render(<ExecutionStart run={run} />);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.start' }));
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.confirm' }));
    await screen.findByText('productFactory.execution.pending');
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
    expect(screen.queryByRole('button', { name: 'productFactory.execution.start' })).not.toBeInTheDocument();
  });

  it('unavailable receipt data fails closed and only allows a read retry', async () => {
    readFailure = true;
    render(<ExecutionStart run={run} />);
    await screen.findByText('productFactory.execution.readError');
    expect(screen.queryByRole('button', { name: 'productFactory.execution.start' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.workspace.retry' }));
    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('explains that execution is blocked when the budget cost is unknown', async () => {
    startCode = 'PRODUCT_FACTORY_COST_UNKNOWN';
    render(<ExecutionStart run={run} />);
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.start' }));
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.execution.confirm' }));
    await screen.findByText('productFactory.execution.costUnknown');
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
  });
});
