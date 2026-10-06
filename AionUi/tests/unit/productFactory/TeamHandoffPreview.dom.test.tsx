/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import ProductFactoryPage from '@/renderer/pages/product-factory';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params?.taskId ? `${key}: ${params.taskId}${params.dependency ? ` -> ${params.dependency}` : ''}` : key,
  }),
}));
vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'test-csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));

const task = {
  id: 'data-1',
  title: 'Define storage',
  description: 'Persist the data contract',
  type: 'data',
  blocked_by: [] as string[],
  acceptance_criteria: ['Survives restart'],
  suggested_role: 'backend',
  effort: 'medium',
};
const draft = { version: 1, generated_by: 'draft', confirmed: true, revision: 3, updated_at: 0, tasks: [task] };
const run = {
  id: 'run-1',
  name: 'Research Copilot',
  idea: 'Turn notes into a plan',
  target_user: 'Product managers',
  problem: 'Scattered notes',
  expected_output: 'A plan',
  workspace_path: '/workspace/project',
  status: 'task_draft_ready',
  task_draft: draft,
  team_id: undefined as string | undefined,
  created_at: 0,
  updated_at: 0,
};
const response = (data: unknown, status = 200) => new Response(JSON.stringify({ data }), { status });
let initial: typeof run;
let latest: typeof run;
let metadataStatus: number;
let directory: boolean;
let requests: Array<{ path: string; method: string; body: unknown }>;
let delayLatest: Promise<void> | undefined;

const openPreview = async () => {
  render(
    <MemoryRouter initialEntries={['/product-factory/run-1']}>
      <ProductFactoryPage />
    </MemoryRouter>
  );
  fireEvent.click(await screen.findByRole('button', { name: 'productFactory.handoff.open' }));
  return screen.findByRole('region', { name: 'productFactory.handoff.title' });
};
const expectBlocked = async (reason: string) => {
  await waitFor(() => expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.blocked'));
  expect(screen.getByTestId('handoff-reasons')).toHaveTextContent(`productFactory.handoff.reasons.${reason}`);
};

describe('Team handoff preview through the real page and API client', () => {
  beforeEach(() => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn((media: string) => ({
        matches: false,
        media,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }))
    );
    initial = structuredClone(run);
    latest = structuredClone(run);
    metadataStatus = 200;
    directory = true;
    requests = [];
    delayLatest = undefined;
    window.localStorage.clear();
    vi.stubGlobal(
      'fetch',
      vi.fn(async (path: string, init?: RequestInit) => {
        const method = init?.method ?? 'GET';
        requests.push({ path, method, body: init?.body ? JSON.parse(String(init.body)) : undefined });
        if (path === '/api/product-factory/runs/run-1' && method === 'GET') {
          if (requests.filter((r) => r.path === path).length === 1) return response(initial);
          await delayLatest;
          return response(latest);
        }
        if (path === '/api/fs/metadata' && method === 'POST') {
          return response({ is_directory: directory, path: latest.workspace_path }, metadataStatus);
        }
        if (path === '/api/product-factory/runs/run-1/planning' && method === 'GET') return response([]);
        if (path === '/api/product-factory/runs/run-1/versions' && method === 'GET')
          return response({ product: null, current_version_id: null, versions: [], operations: [] });
        throw new Error(`Unexpected non-preview request: ${method} ${path}`);
      })
    );
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it('shows every confirmed task field and a ready result without creating a Team or starting an Agent', async () => {
    initial.task_draft.tasks.push({
      ...task,
      id: 'test-1',
      title: 'Verify storage',
      type: 'test',
      blocked_by: ['data-1'],
    });
    latest = structuredClone(initial);
    const preview = await openPreview();
    await waitFor(() => expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.ready'));
    for (const value of [
      'Research Copilot',
      '/workspace/project',
      'Define storage',
      'Persist the data contract',
      'Survives restart',
      'Verify storage',
    ]) {
      expect(within(preview).getAllByText(value).length).toBeGreaterThan(0);
    }
    for (const label of [
      'productName',
      'status',
      'workspace',
      'revision',
      'confirmed',
      'taskCount',
      'handedOff',
      'id',
      'titleField',
      'description',
      'type',
      'dependencies',
      'criteria',
      'role',
      'effort',
    ]) {
      expect(within(preview).getAllByText(`productFactory.handoff.fields.${label}`).length).toBeGreaterThan(0);
    }
    expect(requests).toEqual([
      { path: '/api/product-factory/runs/run-1', method: 'GET', body: undefined },
      { path: '/api/product-factory/runs/run-1/versions', method: 'GET', body: undefined },
      { path: '/api/product-factory/runs/run-1/planning', method: 'GET', body: undefined },
      { path: '/api/product-factory/runs/run-1/cost-report', method: 'GET', body: undefined },
      { path: '/api/product-factory/runs/run-1', method: 'GET', body: undefined },
      {
        path: '/api/fs/metadata',
        method: 'POST',
        body: { path: '/workspace/project', workspace: '/workspace/project' },
      },
    ]);
    expect(within(preview).queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('blocks an unconfirmed draft', async () => {
    initial.task_draft.confirmed = false;
    latest = structuredClone(initial);
    await openPreview();
    await expectBlocked('unconfirmed');
  });

  it.each([
    ['missingDependency', { blocked_by: ['missing-1'] }],
    ['selfDependency', { blocked_by: ['data-1'] }],
    ['emptyTitle', { title: '  ' }],
    ['emptyDescription', { description: ' ' }],
    ['emptyCriteria', { acceptance_criteria: [' ', ''] }],
    ['invalidId', { id: 'invalid id' }],
  ])('explains %s with the affected task ID', async (reason, patch) => {
    latest.task_draft.tasks = [{ ...task, ...patch }];
    await openPreview();
    await expectBlocked(reason as string);
    expect(screen.getByTestId('handoff-reasons')).toHaveTextContent(latest.task_draft.tasks[0].id);
  });

  it.each([
    ['duplicateId', [{ ...task }, { ...task }]],
    ['duplicateDependency', [{ ...task }, { ...task, id: 'test-1', blocked_by: ['data-1', 'data-1'] }]],
    [
      'cycle',
      [
        { ...task, blocked_by: ['test-1'] },
        { ...task, id: 'test-1', blocked_by: ['data-1'] },
      ],
    ],
  ])('rejects %s in the dependency graph', async (reason, tasks) => {
    latest.task_draft.tasks = tasks as (typeof task)[];
    await openPreview();
    await expectBlocked(reason as string);
  });

  it('rejects a missing server draft', async () => {
    latest = { ...latest, task_draft: undefined } as unknown as typeof run;
    await openPreview();
    await expectBlocked('missingDraft');
  });

  it('rejects an illegal source rather than normalizing it into a legal source', async () => {
    latest.task_draft.generated_by = 'model';
    await openPreview();
    await expectBlocked('invalidSource');
  });

  it('rejects an empty task list', async () => {
    latest.task_draft.tasks = [];
    await openPreview();
    await expectBlocked('emptyTasks');
  });

  it('rejects a stale revision without overwriting the local editor', async () => {
    initial.task_draft.confirmed = false;
    latest.task_draft.revision = 4;
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    fireEvent.change(await screen.findByPlaceholderText('productFactory.taskDraft.titleField'), {
      target: { value: 'My local title' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.open' }));
    await expectBlocked('staleRevision');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.close' }));
    expect(screen.getByPlaceholderText('productFactory.taskDraft.titleField')).toHaveValue('My local title');
  });

  it.each([0, -1, 1.5])('rejects invalid revision %s', async (revision) => {
    latest.task_draft.revision = revision;
    await openPreview();
    await expectBlocked('invalidRevision');
  });

  it.each([
    ['missingWorkspace', ''],
    ['invalidWorkspace', 'relative/path'],
  ])('rejects %s without a metadata request', async (reason, path) => {
    initial.workspace_path = path;
    latest = structuredClone(initial);
    await openPreview();
    await expectBlocked(reason);
    expect(requests.every((r) => r.method === 'GET')).toBe(true);
  });

  it.each([
    [404, 'workspaceNotFound'],
    [403, 'workspaceUnavailable'],
    [500, 'workspaceUnavailable'],
  ])('fails closed on metadata HTTP %s', async (status, reason) => {
    metadataStatus = status as number;
    await openPreview();
    await expectBlocked(reason as string);
  });

  it('rejects a file used as a workspace', async () => {
    directory = false;
    await openPreview();
    await expectBlocked('workspaceNotDirectory');
  });

  it.each(['handed_off', 'running', 'in_review', 'completed'])('rejects a run already in %s', async (status) => {
    latest.status = status;
    await openPreview();
    await expectBlocked('alreadyHandedOff');
  });

  it('rejects an existing team even when the status is task_draft_ready', async () => {
    latest.team_id = 'existing-team';
    await openPreview();
    await expectBlocked('alreadyHandedOff');
  });

  it('does not show ready while waiting for the current server revision', async () => {
    let resolve!: () => void;
    delayLatest = new Promise<void>((done) => {
      resolve = done;
    });
    await openPreview();
    expect(screen.getByTestId('handoff-result')).not.toHaveTextContent('productFactory.handoff.ready');
    await act(async () => {
      resolve();
    });
    await waitFor(() => expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.ready'));
  });

  it('refreshes the server check and clears a previous ready result on failure', async () => {
    await openPreview();
    await waitFor(() => expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.ready'));
    vi.mocked(fetch).mockRejectedValueOnce(new Error('Offline'));
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.refresh' }));
    await expectBlocked('serverUnavailable');
  });
});
