import React from 'react';
import { Button } from '@arco-design/web-react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TeamAssistantInput } from '@/common/adapter/teamMapper';
import PlanningPanel from '@/renderer/pages/product-factory/components/Preparation/Planning';
import { apiRun, attempt, costReport, run } from './fixtures';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, params?: { code?: string }) => (params?.code ? `${key}: ${params.code}` : key),
  }),
}));
vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));
vi.mock('@/renderer/pages/team/components/TeamCreateModal', () => ({
  default: ({
    creationOverride,
  }: {
    creationOverride: { submit: (agents: TeamAssistantInput[]) => Promise<void>; confirmLabel?: string };
  }) => (
    <Button
      onClick={() =>
        void creationOverride
          .submit([{ assistant_id: 'configured-assistant', assistant_name: 'Planner', role: 'leader', model: '' }])
          .catch(() => undefined)
      }
    >
      {creationOverride.confirmLabel}
    </Button>
  ),
}));

type Call = { path: string; method: string; body?: Record<string, unknown> };
let calls: Call[];
let persisted: (typeof attempt)[];
let generationState: string;
let loseResponse: boolean;
let readFailure: boolean;
let generationFailure: string | undefined;
const response = (data: unknown, status = 200) => new Response(JSON.stringify({ success: true, data }), { status });
const onApplied = vi.fn();
const onActiveChange = vi.fn();
const mount = (disabled = false) =>
  render(<PlanningPanel run={run} disabled={disabled} onApplied={onApplied} onActiveChange={onActiveChange} />);
const ready = () =>
  waitFor(() => expect(screen.getByRole('button', { name: 'productFactory.planning.generate' })).not.toBeDisabled());

describe('durable model planning', () => {
  beforeEach(() => {
    calls = [];
    persisted = [];
    generationState = 'candidate_ready';
    loseResponse = false;
    readFailure = false;
    generationFailure = undefined;
    vi.clearAllMocks();
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
        if (path.endsWith('/planning') && method === 'GET')
          return readFailure ? response(null, 500) : response(persisted);
        if (path.endsWith('/cost-report') && method === 'GET') return response(costReport);
        if (path.endsWith('/planning') && method === 'POST') {
          if (generationFailure)
            return new Response(JSON.stringify({ error: 'Planning is blocked', code: generationFailure }), {
              status: 409,
            });
          persisted = [
            {
              ...attempt,
              state: generationState,
              idempotency_key: String(body?.idempotency_key),
              candidate: generationState === 'candidate_ready' ? attempt.candidate : null,
            },
          ];
          if (loseResponse) throw new Error('response lost after accepted');
          return response(persisted[0], 202);
        }
        if (path.endsWith('/apply') && method === 'POST') {
          persisted = persisted.map((item) => Object.assign({}, item, { state: 'applied' }));
          return response({ ...apiRun, plan_revision: 2, status: 'interviewing', interview: attempt.candidate });
        }
        throw new Error(`unexpected ${method} ${path}`);
      })
    );
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('restores records on opening without generating a model turn', async () => {
    persisted = [attempt];
    mount();
    expect(await screen.findByText('Who will use it?')).toBeInTheDocument();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('generates once only after assistant selection and explicit confirmation', async () => {
    mount();
    await ready();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.planning.generate' }));
    expect(calls.some((call) => call.method === 'POST')).toBe(false);
    const confirm = await screen.findByRole('button', { name: 'productFactory.planning.confirmGeneration' });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await screen.findByText('Who will use it?');
    const generation = calls.filter((call) => call.path.endsWith('/planning') && call.method === 'POST');
    expect(generation).toHaveLength(1);
    expect(generation[0].body).toMatchObject({
      phase: 'interview',
      expected_plan_revision: 1,
      assistant_id: 'configured-assistant',
      model: '',
      idempotency_key: expect.any(String),
    });
  });

  it('applies an unconfirmed candidate without calling any confirmation endpoint', async () => {
    persisted = [attempt];
    mount();
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.planning.apply' }));
    await waitFor(() =>
      expect(onApplied).toHaveBeenCalledWith(
        expect.objectContaining({ interview: expect.objectContaining({ confirmed: false }), planRevision: 2 })
      )
    );
    expect(calls.some((call) => call.path.endsWith('/confirm'))).toBe(false);
    expect(calls.find((call) => call.path.endsWith('/apply'))?.body).toEqual({ expected_plan_revision: 1 });
  });

  it('recovers a lost generation response with reads and never resends it', async () => {
    generationState = 'running';
    loseResponse = true;
    mount();
    await ready();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.planning.generate' }));
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.planning.confirmGeneration' }));
    await screen.findByRole('button', { name: 'productFactory.planning.cancel' });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.planning.refresh' }));
    await waitFor(() =>
      expect(calls.filter((call) => call.method === 'GET' && call.path.endsWith('/planning')).length).toBeGreaterThan(2)
    );
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
  });

  it('keeps uncertain attempts visible and blocks generation after refresh', async () => {
    persisted = [{ ...attempt, state: 'uncertain', candidate: null }];
    mount();
    await screen.findByText('productFactory.planning.uncertainHint');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.planning.refresh' }));
    expect(screen.getByRole('button', { name: 'productFactory.planning.generate' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'productFactory.planning.retry' })).not.toBeInTheDocument();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('polls active attempts only while visible and stops at the terminal state', async () => {
    persisted = [{ ...attempt, state: 'running', candidate: null }];
    vi.useFakeTimers();
    await act(async () => {
      mount();
    });
    const initial = calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(calls.length).toBeGreaterThan(initial);
    vi.mocked(vi.spyOn(document, 'visibilityState', 'get')).mockReturnValue('hidden');
    const before = calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(calls).toHaveLength(before);
    vi.mocked(vi.spyOn(document, 'visibilityState', 'get')).mockReturnValue('visible');
    persisted = [{ ...attempt, state: 'failed', candidate: null, error_code: 'PLANNING_OUTPUT_INVALID' }];
    await act(async () => {
      document.dispatchEvent(new Event('visibilitychange'));
    });
    const stopped = calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10000);
    });
    expect(calls).toHaveLength(stopped);
  });

  it('fails closed when durable planning records cannot be read', async () => {
    readFailure = true;
    mount();
    await screen.findByText('productFactory.planning.readError');
    expect(screen.getByRole('button', { name: 'productFactory.planning.generate' })).toBeDisabled();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('shows the real budget blocker and keeps unpriced amounts unknown', async () => {
    generationFailure = 'PLANNING_COST_UNKNOWN';
    mount();
    await ready();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.planning.generate' }));
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.planning.confirmGeneration' }));
    expect(await screen.findByText('productFactory.planning.operationCode: PLANNING_COST_UNKNOWN')).toBeInTheDocument();
    expect(screen.queryByText('$0.0000')).not.toBeInTheDocument();
    expect(calls.filter((call) => call.method === 'POST')).toHaveLength(1);
  });

  it('explains local Send rejection without mislabelling it as missing model prices', async () => {
    persisted = [{ ...attempt, state: 'failed', candidate: null, error_code: 'USER_MODEL_SEND_ADMISSION_REJECTED' }];
    mount();
    expect(await screen.findByText('productFactory.planning.errors.sendAdmissionRejected')).toBeInTheDocument();
    expect(calls.every((call) => call.method === 'GET')).toBe(true);
  });

  it('keeps candidate application and generation disabled for unsaved edits', async () => {
    persisted = [attempt];
    mount(true);
    expect(await screen.findByRole('button', { name: 'productFactory.planning.apply' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'productFactory.planning.generate' })).toBeDisabled();
  });
});
