/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React from 'react';
import { Button } from '@arco-design/web-react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import ProductFactoryPage from '@/renderer/pages/product-factory';
import type { TeamAssistantInput } from '@/common/adapter/teamMapper';
import { handoffProductFactoryRun } from '@/renderer/pages/product-factory/client';

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'test-csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));
vi.mock('@/renderer/pages/team/components/TeamCreateModal', () => ({
  default: ({
    visible,
    creationOverride,
  }: {
    visible: boolean;
    creationOverride: { submit: (agents: TeamAssistantInput[]) => Promise<void> };
  }) =>
    visible ? (
      <Button
        onClick={() =>
          void creationOverride
            .submit([
              {
                assistant_id: 'configured-assistant',
                assistant_name: 'Developer',
                role: 'leader',
                model: 'configured-model',
              },
            ])
            .catch(() => undefined)
        }
      >
        Confirm selected members
      </Button>
    ) : null,
}));

const fixture = {
  id: 'run-1',
  name: 'Product',
  idea: 'Build a product',
  target_user: '',
  problem: '',
  expected_output: '',
  workspace_path: '/workspace/product',
  status: 'task_draft_ready',
  interview: null as unknown,
  blueprint: null as unknown,
  team_id: null as string | null,
  created_at: 0,
  updated_at: 0,
  task_draft: {
    version: 1,
    generated_by: 'draft',
    confirmed: true,
    revision: 2,
    updated_at: 0,
    tasks: [
      {
        id: 'task-1',
        title: 'Build',
        description: 'Core workflow',
        type: 'backend',
        blocked_by: [],
        acceptance_criteria: ['Pass review'],
        suggested_role: 'backend',
        effort: 'medium',
      },
    ],
  },
};
let run: typeof fixture;
let newlyCreatedRun: typeof fixture | undefined;
let createFailure: boolean;
let failure: string | undefined;
let calls: { path: string; method: string; body?: unknown; csrf: string | null }[];
const response = (data: unknown) => new Response(JSON.stringify({ data }), { status: 200 });
const open = async () => {
  render(
    <MemoryRouter initialEntries={['/product-factory/run-1']}>
      <Routes>
        <Route path='/product-factory/:id' element={<ProductFactoryPage />} />
        <Route path='/team/:id' element={<div data-testid='existing-team-page' />} />
      </Routes>
    </MemoryRouter>
  );
  fireEvent.click(await screen.findByRole('button', { name: 'productFactory.handoff.open' }));
  await waitFor(() => expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.ready'));
};

describe('explicit Product Factory Team handoff', () => {
  beforeEach(() => {
    run = structuredClone(fixture);
    newlyCreatedRun = undefined;
    createFailure = false;
    calls = [];
    failure = undefined;
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
        if (path === '/api/product-factory/runs/run-1' && method === 'GET') return response(run);
        if (path === '/api/product-factory/runs/run-1/planning' && method === 'GET') return response([]);
        if (path === '/api/product-factory/runs/run-1/versions' && method === 'GET')
          return response({ product: null, current_version_id: null, versions: [], operations: [] });
        if (path === '/api/product-factory/runs/run-1/execution' && method === 'GET')
          return response({ execution: null });
        if (path === '/api/product-factory/runs/run-1/interview' && method === 'PUT') {
          run = { ...run, status: 'interviewing', interview: JSON.parse(String(init?.body)).interview };
          return response(run);
        }
        if (path === '/api/product-factory/runs/run-1/blueprint' && method === 'PUT') {
          run = { ...run, status: 'blueprint_ready', blueprint: JSON.parse(String(init?.body)).blueprint };
          return response(run);
        }
        if (path === '/api/product-factory/runs' && method === 'GET')
          return response(newlyCreatedRun ? [newlyCreatedRun, run] : [run]);
        if (path === '/api/product-factory/runs' && method === 'POST') {
          if (createFailure)
            return new Response(JSON.stringify({ code: 'BAD_REQUEST', error: 'Cannot create new run' }), {
              status: 400,
            });
          const submitted = JSON.parse(String(init?.body)) as { name: string; idea: string; workspace_path: string };
          newlyCreatedRun = {
            ...structuredClone(fixture),
            id: 'new-run',
            name: submitted.name,
            idea: submitted.idea,
            workspace_path: submitted.workspace_path,
            status: 'draft',
            task_draft: undefined as never,
          };
          return response(newlyCreatedRun);
        }
        if (path === '/api/fs/metadata') return response({ is_directory: true });
        if (path === '/api/product-factory/runs/run-1/handoff') {
          if (failure)
            return new Response(JSON.stringify({ code: failure, error: 'server rejected request' }), { status: 409 });
          run = { ...run, status: 'handed_off', team_id: 'created-team' };
          return response({ run, team_id: 'created-team', task_id_map: { 'task-1': 'actual-task-1' } });
        }
        throw new Error(`Unexpected request ${method} ${path}`);
      })
    );
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it('creates through the authoritative endpoint only after explicit member confirmation', async () => {
    await open();
    expect(calls.some((call) => call.path.endsWith('/handoff'))).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.execution.configure' }));
    expect(calls.some((call) => call.path.endsWith('/handoff'))).toBe(false);
    fireEvent.click(await screen.findByRole('button', { name: 'Confirm selected members' }));
    expect(await screen.findByText('productFactory.handoff.execution.success')).toBeInTheDocument();
    const handoffCall = calls.find((call) => call.path.endsWith('/handoff'))!;
    expect(handoffCall.body).toEqual({
      expected_revision: 2,
      agents: [{ name: 'Developer', role: 'lead', assistant_id: 'configured-assistant', model: 'configured-model' }],
    });
    expect(handoffCall.csrf).toBe('test-csrf');
    expect(calls.some((call) => call.path.startsWith('/api/teams') || call.path.includes('/messages'))).toBe(false);
    expect(screen.queryByTestId('existing-team-page')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.execution.openTeam' }));
    expect(await screen.findByTestId('existing-team-page')).toBeInTheDocument();
  });

  it('retains the confirmed draft and offers retry when the backend rejects its revision', async () => {
    failure = 'PRODUCT_FACTORY_REVISION_CONFLICT';
    await open();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.handoff.execution.configure' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Confirm selected members' }));
    expect(await screen.findByText('productFactory.handoff.execution.revisionError')).toBeInTheDocument();
    expect(run.task_draft.revision).toBe(2);
    expect(run.team_id).toBeNull();
    failure = undefined;
    fireEvent.click(screen.getByRole('button', { name: 'Confirm selected members' }));
    expect(await screen.findByText('productFactory.handoff.execution.success')).toBeInTheDocument();
  });

  it('restores the existing Team entry from a previously handed-off run without creating another Team', async () => {
    run = { ...run, status: 'handed_off', team_id: 'created-team' };
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    expect(
      await screen.findByRole('button', { name: 'productFactory.handoff.execution.openTeam' })
    ).toBeInTheDocument();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('keeps the first-task start action available before any dispatch receipt exists', async () => {
    run = { ...run, status: 'handed_off', team_id: 'created-team' };
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    expect(await screen.findByRole('button', { name: 'productFactory.execution.start' })).toBeInTheDocument();
    expect(calls.some((call) => call.path.endsWith('/review'))).toBe(false);
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('does not offer member confirmation when the draft is unconfirmed', async () => {
    run.task_draft.confirmed = false;
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.handoff.open' }));
    await waitFor(() =>
      expect(screen.getByTestId('handoff-result')).toHaveTextContent('productFactory.handoff.blocked')
    );
    expect(
      screen.queryByRole('button', { name: 'productFactory.handoff.execution.configure' })
    ).not.toBeInTheDocument();
    expect(calls.every((call) => !call.path.endsWith('/handoff'))).toBe(true);
  });

  it('preserves an intentionally empty model supplied by the existing assistant resolver', async () => {
    await handoffProductFactoryRun('run-1', 2, [
      { assistant_id: 'configured-assistant', assistant_name: 'Developer', role: 'leader', model: '' },
    ]);
    expect(calls[0].body).toEqual({
      expected_revision: 2,
      agents: [{ name: 'Developer', role: 'lead', assistant_id: 'configured-assistant', model: '' }],
    });
  });

  it('creates a new run from the idea form even when an older run is selected', async () => {
    render(
      <MemoryRouter initialEntries={['/product-factory']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    await screen.findByText('Product');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.home.createProduct' }));
    expect(screen.getByTestId('product-factory-workspace')).toHaveValue('');
    expect(screen.getByTestId('product-factory-name')).toHaveValue('');
    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'New Product' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: 'A distinct idea' } });
    fireEvent.change(screen.getByTestId('product-factory-workspace'), { target: { value: '/workspace/new' } });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.form.startAnalysis' }));
    await waitFor(() =>
      expect(calls.filter((call) => call.path === '/api/product-factory/runs' && call.method === 'POST')).toHaveLength(
        1
      )
    );
    expect(newlyCreatedRun).toMatchObject({ id: 'new-run', name: 'New Product', workspace_path: '/workspace/new' });
    expect(run.name).toBe('Product');
  });

  it('keeps the new idea on failed creation and can retry without touching the older run', async () => {
    createFailure = true;
    render(
      <MemoryRouter initialEntries={['/product-factory']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    await screen.findByText('Product');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.home.createProduct' }));
    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'Retry Product' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: 'Keep the idea' } });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.form.startAnalysis' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Cannot create new run');
    expect(screen.getByTestId('product-factory-name')).toHaveValue('Retry Product');
    expect(run.name).toBe('Product');
    createFailure = false;
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.form.startAnalysis' }));
    await waitFor(() =>
      expect(calls.filter((call) => call.path === '/api/product-factory/runs' && call.method === 'POST')).toHaveLength(
        2
      )
    );
    expect(newlyCreatedRun?.name).toBe('Retry Product');
  });

  it('keeps interview answers in the rendered run and persists the current text', async () => {
    run.status = 'draft';
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    const answer = await screen.findByTestId('product-factory-interview-answer-audience');
    fireEvent.change(answer, { target: { value: 'Product managers' } });
    expect(answer).toHaveValue('Product managers');
    fireEvent.change(screen.getByTestId('product-factory-interview-summary'), {
      target: { value: 'One reviewable plan and Team graph' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.interview.save' }));
    await waitFor(() =>
      expect(calls.some((call) => call.path.endsWith('/interview') && call.method === 'PUT')).toBe(true)
    );
    const saved = calls.find((call) => call.path.endsWith('/interview') && call.method === 'PUT')!.body as {
      interview: {
        questions: { answer: string }[];
        summary: string;
      };
    };
    expect(saved.interview.questions[0].answer).toBe('Product managers');
    expect(saved.interview.summary).toBe('One reviewable plan and Team graph');
  });

  it('keeps blueprint section edits in the rendered run and persists them', async () => {
    run.status = 'blueprint_ready';
    run.blueprint = {
      version: 1,
      sections: [{ id: 'goals', title: 'Goal', content: 'Initial content' }],
      risks: [],
      open_questions: [],
      confirmed: false,
    };
    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );
    const content = await screen.findByDisplayValue('Initial content');
    fireEvent.change(content, { target: { value: 'Edited content' } });
    expect(content).toHaveValue('Edited content');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.blueprint.save' }));
    await waitFor(() =>
      expect(calls.some((call) => call.path.endsWith('/blueprint') && call.method === 'PUT')).toBe(true)
    );
    const saved = calls.find((call) => call.path.endsWith('/blueprint') && call.method === 'PUT')!.body as {
      blueprint: { sections: { content: string }[] };
    };
    expect(saved.blueprint.sections[0].content).toBe('Edited content');
  });
});
