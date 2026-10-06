import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  applyProductFactoryPlanning,
  getProductFactoryCostReport,
  getProductFactoryRun,
  listProductFactoryPlanning,
  saveProductFactoryTaskDraft,
  startProductFactoryPlanning,
} from '@/renderer/pages/product-factory/client';
import { apiRun, attempt, costReport, v2Draft } from './fixtures';
import type { TaskDraftArtifact } from '@/renderer/pages/product-factory/types';

vi.mock('@/common/adapter/httpBridge', () => ({ getBaseUrl: () => '', resolveCoreCsrfToken: () => 'csrf' }));
vi.mock('@/common/adapter/sessionRefresh', () => ({ refreshSession: vi.fn() }));
const response = (data: unknown) => new Response(JSON.stringify({ success: true, data }));

describe('planning and task schema API boundary', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('preserves model source and explicit v2 execution scopes from the server', async () => {
    vi.mocked(fetch).mockResolvedValue(response({ ...apiRun, task_draft: v2Draft }));
    const loaded = await getProductFactoryRun('run-1');
    expect(loaded.taskDraft).toMatchObject({
      version: 2,
      generatedBy: 'model',
      tasks: [
        expect.objectContaining({ executionScope: 'task_workspace', requirementIds: ['req-1'] }),
        expect.objectContaining({ executionScope: 'task_workspace' }),
        expect.objectContaining({ executionScope: 'project_integration' }),
      ],
    });
  });

  it('keeps a historical v1 rules draft editable without inventing v2 permissions', async () => {
    const legacy = { ...v2Draft, version: 1, generated_by: 'draft', tasks: [v2Draft.tasks[0]] };
    vi.mocked(fetch).mockResolvedValue(response({ ...apiRun, task_draft: legacy }));
    expect((await getProductFactoryRun('run-1')).taskDraft).toMatchObject({ version: 1, generatedBy: 'draft' });
  });

  it.each([
    { ...v2Draft, version: 1 },
    { ...v2Draft, generated_by: 'draft' },
    { ...v2Draft, tasks: [{ ...v2Draft.tasks[0], execution_scope: 'anywhere' }] },
  ])('rejects unsupported source/version or scope instead of relabelling it', async (draft) => {
    vi.mocked(fetch).mockResolvedValue(response({ ...apiRun, task_draft: draft }));
    await expect(getProductFactoryRun('run-1')).rejects.toThrow('PRODUCT_FACTORY_INVALID_TASK_DRAFT');
  });

  it('serializes v2 permission and requirement links when a human saves an edit', async () => {
    vi.mocked(fetch).mockResolvedValueOnce(response({ ...apiRun, task_draft: v2Draft }));
    const loaded = await getProductFactoryRun('run-1');
    vi.mocked(fetch).mockResolvedValueOnce(response({ ...apiRun, task_draft: v2Draft }));
    await saveProductFactoryTaskDraft('run-1', loaded.taskDraft as TaskDraftArtifact);
    const options = vi.mocked(fetch).mock.calls[1][1];
    expect(JSON.parse(String(options?.body)).tasks[2]).toMatchObject({
      execution_scope: 'project_integration',
      requirement_ids: ['req-1'],
    });
  });

  it('sends the exact configured assistant/default model and a durable generation key', async () => {
    vi.mocked(fetch).mockResolvedValue(response(attempt));
    await startProductFactoryPlanning('run-1', {
      phase: 'interview',
      expectedPlanRevision: 1,
      idempotencyKey: 'one-key',
      assistantId: 'configured-assistant',
      model: '',
    });
    expect(JSON.parse(String(vi.mocked(fetch).mock.calls[0][1]?.body))).toEqual({
      phase: 'interview',
      expected_plan_revision: 1,
      idempotency_key: 'one-key',
      assistant_id: 'configured-assistant',
      model: '',
    });
  });

  it('does not confirm a candidate when applying it', async () => {
    vi.mocked(fetch).mockResolvedValue(response({ ...apiRun, interview: attempt.candidate }));
    const loaded = await applyProductFactoryPlanning('run-1', 'attempt-1', 1);
    expect(loaded.interview).toMatchObject({ confirmed: false });
    expect(vi.mocked(fetch).mock.calls[0][0]).toBe('/api/product-factory/runs/run-1/planning/attempt-1/apply');
  });

  it('rejects an unrecognized planning state instead of treating it as safe to generate again', async () => {
    vi.mocked(fetch).mockResolvedValue(response([{ ...attempt, state: 'silently_done' }]));
    await expect(listProductFactoryPlanning('run-1')).rejects.toThrow('PRODUCT_FACTORY_INVALID_PLANNING_RESPONSE');
  });

  it('keeps planning provenance, unique Send attempts and unknown amounts in the cost report', async () => {
    const usage = {
      id: 'u',
      input_tokens: 1,
      output_tokens: 2,
      cost_est: null,
      conversation_id: 'c',
      turn_id: 't',
      attempt_id: 'send-1',
      cost_unknown_reason: 'model_not_priced',
      created_at: 1,
    };
    vi.mocked(fetch).mockResolvedValue(
      response({
        ...costReport,
        planning_usage: [
          { planning_attempt_id: 'attempt-1', phase: 'interview', state: 'failed', cost_unknown: true, usage: [usage] },
        ],
      })
    );
    const report = await getProductFactoryCostReport('run-1');
    expect(report.planningUsage?.[0]).toMatchObject({
      planningAttemptId: 'attempt-1',
      costUnknown: true,
      usage: [
        expect.objectContaining({ attemptId: 'send-1', costEst: undefined, costUnknownReason: 'model_not_priced' }),
      ],
    });
    expect(report.usageSummary.costEst).toBeUndefined();
  });
});
