/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { getBaseUrl, resolveCoreCsrfToken } from '@/common/adapter/httpBridge';
import { refreshSession } from '@/common/adapter/sessionRefresh';
import { asRecord, normalizePreviewDraft, workspacePathIssue, type HandoffIssueCode } from './handoffPreview';
import { toBackendAssistant, type TeamAssistantInput } from '@/common/adapter/teamMapper';
import type {
  BlueprintArtifact,
  InterviewArtifact,
  ProductFactoryRun,
  ProductIdeaDraft,
  TaskDraftArtifact,
  TaskDraftTask,
  FactoryExecution,
  FactoryTask,
  FactoryTaskUsage,
  ProductFactoryDelivery,
  ProductFactoryCostReport,
  ProductFactoryReview,
  PlanningAttempt,
  PlanningPhase,
  PlanningState,
} from './types';

type ApiEnvelope<T> = { success?: boolean; data?: T; error?: string; code?: string };

export class ProductFactoryHttpError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly code?: string
  ) {
    super(message);
  }
}

type ApiRun = {
  id: string;
  name: string;
  idea: string;
  target_user: string;
  problem: string;
  expected_output: string;
  workspace_path: string;
  budget_usd?: number;
  status: ProductFactoryRun['status'];
  interview?: unknown;
  blueprint?: unknown;
  task_draft?: unknown;
  team_id?: string;
  created_at: number;
  updated_at: number;
  plan_revision?: number;
};

type ApiTaskDraft = {
  version: number;
  generated_by: string;
  confirmed: boolean;
  revision: number;
  updated_at: number;
  tasks: Array<{
    id: string;
    title: string;
    description: string;
    type: string;
    blocked_by: string[];
    acceptance_criteria: string[];
    suggested_role: string;
    effort: string;
    execution_scope?: string;
    requirement_ids?: string[];
  }>;
};

const parseTaskDraft = (value: unknown): TaskDraftArtifact | undefined => {
  if (value == null) return undefined;
  if (!value || typeof value !== 'object') throw new Error('PRODUCT_FACTORY_INVALID_TASK_DRAFT');
  const raw = value as ApiTaskDraft;
  const types = new Set(['frontend', 'backend', 'data', 'test']);
  const efforts = new Set(['low', 'medium', 'high']);
  if (
    ![1, 2].includes(raw.version) ||
    !['draft', 'model'].includes(raw.generated_by) ||
    (raw.version === 1 && raw.generated_by !== 'draft') ||
    (raw.version === 2 && raw.generated_by !== 'model') ||
    !Array.isArray(raw.tasks)
  ) {
    throw new Error('PRODUCT_FACTORY_INVALID_TASK_DRAFT');
  }
  const tasks: TaskDraftTask[] = raw.tasks.map((task) => {
    if (
      !task ||
      typeof task.id !== 'string' ||
      typeof task.title !== 'string' ||
      typeof task.description !== 'string' ||
      !types.has(task.type) ||
      !efforts.has(task.effort) ||
      !Array.isArray(task.blocked_by) ||
      !Array.isArray(task.acceptance_criteria) ||
      (raw.version === 2 &&
        (!['task_workspace', 'project_integration'].includes(task.execution_scope ?? '') ||
          !Array.isArray(task.requirement_ids) ||
          !task.requirement_ids.every((id) => typeof id === 'string')))
    ) {
      throw new Error('PRODUCT_FACTORY_INVALID_TASK_DRAFT');
    }
    return {
      id: task.id,
      title: task.title,
      description: task.description,
      type: task.type as TaskDraftTask['type'],
      blockedBy: task.blocked_by,
      acceptanceCriteria: task.acceptance_criteria,
      suggestedRole: task.suggested_role,
      effort: task.effort as TaskDraftTask['effort'],
      ...(raw.version === 2
        ? {
            executionScope: task.execution_scope as TaskDraftTask['executionScope'],
            requirementIds: task.requirement_ids,
          }
        : {}),
    };
  });
  return {
    version: raw.version as 1 | 2,
    generatedBy: raw.generated_by as 'draft' | 'model',
    confirmed: raw.confirmed === true,
    revision: raw.revision,
    updatedAt: raw.updated_at,
    tasks,
  };
};

export const normalizeRun = (run: ApiRun, preview = false): ProductFactoryRun => ({
  id: run.id,
  name: run.name,
  idea: run.idea,
  targetUser: run.target_user,
  problem: run.problem,
  expectedOutput: run.expected_output,
  workspacePath: run.workspace_path,
  budgetUsd: run.budget_usd,
  status: run.status,
  interview: run.interview,
  blueprint: run.blueprint,
  taskDraft: preview ? normalizePreviewDraft(run.task_draft) : parseTaskDraft(run.task_draft),
  teamId: run.team_id,
  createdAt: run.created_at,
  updatedAt: run.updated_at,
  planRevision: run.plan_revision ?? 1,
});

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const csrf =
    resolveCoreCsrfToken() ||
    (typeof document !== 'undefined'
      ? document.cookie
          .split('; ')
          .find((cookie) => cookie.startsWith('aionui-csrf-token='))
          ?.split('=')
          .slice(1)
          .join('=')
      : undefined);
  const headers = new Headers(init.headers);
  headers.set('Content-Type', 'application/json');
  if (csrf) headers.set('x-csrf-token', csrf);
  const options: RequestInit = { ...init, headers, credentials: 'same-origin' };
  let response = await fetch(`${getBaseUrl()}${path}`, options);
  if (response.status === 401) {
    await refreshSession();
    response = await fetch(`${getBaseUrl()}${path}`, options);
  }
  const body = (await response.json().catch((): null => null)) as ApiEnvelope<T> | null;
  if (!response.ok || !body?.data) {
    throw new ProductFactoryHttpError(
      body?.error || `PRODUCT_FACTORY_HTTP_${response.status}`,
      response.status,
      body?.code
    );
  }
  return body.data;
}

export async function listProductFactoryRuns(): Promise<ProductFactoryRun[]> {
  const runs = await request<ApiRun[]>('/api/product-factory/runs');
  return runs.map((run) => normalizeRun(run));
}

export async function getProductFactoryRun(runId: string): Promise<ProductFactoryRun> {
  return normalizeRun(await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}`));
}

const PLANNING_PHASES = new Set<PlanningPhase>(['interview', 'blueprint', 'task_graph']);
const PLANNING_STATES = new Set<PlanningState>([
  'reserved',
  'preparing',
  'running',
  'candidate_ready',
  'applied',
  'failed',
  'cancelled',
  'uncertain',
]);

function normalizePlanningAttempt(value: unknown): PlanningAttempt {
  const raw = asRecord(value);
  if (
    typeof raw.id !== 'string' ||
    typeof raw.run_id !== 'string' ||
    !PLANNING_PHASES.has(raw.phase as PlanningPhase) ||
    !PLANNING_STATES.has(raw.state as PlanningState) ||
    !Number.isSafeInteger(raw.input_plan_revision) ||
    typeof raw.idempotency_key !== 'string' ||
    typeof raw.input_hash !== 'string' ||
    typeof raw.assistant_id !== 'string' ||
    typeof raw.model !== 'string' ||
    typeof raw.created_at !== 'number' ||
    typeof raw.updated_at !== 'number' ||
    (raw.candidate != null && (!raw.candidate || typeof raw.candidate !== 'object' || Array.isArray(raw.candidate)))
  )
    throw new Error('PRODUCT_FACTORY_INVALID_PLANNING_RESPONSE');
  return {
    id: raw.id,
    runId: raw.run_id,
    phase: raw.phase as PlanningPhase,
    state: raw.state as PlanningState,
    inputPlanRevision: raw.input_plan_revision as number,
    inputHash: raw.input_hash,
    idempotencyKey: raw.idempotency_key,
    assistantId: raw.assistant_id,
    model: raw.model,
    conversationId: typeof raw.conversation_id === 'string' ? raw.conversation_id : undefined,
    appTurnId: typeof raw.app_turn_id === 'string' ? raw.app_turn_id : undefined,
    candidate: raw.candidate == null ? undefined : asRecord(raw.candidate),
    errorCode: typeof raw.error_code === 'string' ? raw.error_code : undefined,
    createdAt: raw.created_at,
    updatedAt: raw.updated_at,
  };
}

const planningPath = (runId: string) => `/api/product-factory/runs/${encodeURIComponent(runId)}/planning`;

/** Read persisted attempts; never start or replay a model turn. */
export async function listProductFactoryPlanning(runId: string): Promise<PlanningAttempt[]> {
  const attempts = await request<unknown[]>(planningPath(runId), { cache: 'no-store' });
  if (!Array.isArray(attempts)) throw new Error('PRODUCT_FACTORY_INVALID_PLANNING_RESPONSE');
  return attempts.map(normalizePlanningAttempt);
}

export async function getProductFactoryPlanning(runId: string, attemptId: string): Promise<PlanningAttempt> {
  return normalizePlanningAttempt(
    await request<unknown>(`${planningPath(runId)}/${encodeURIComponent(attemptId)}`, { cache: 'no-store' })
  );
}

/** A deliberate generation request; preserve its key across response-loss recovery. */
export async function startProductFactoryPlanning(
  runId: string,
  options: {
    phase: PlanningPhase;
    expectedPlanRevision: number;
    idempotencyKey: string;
    assistantId: string;
    model: string;
  }
): Promise<PlanningAttempt> {
  return normalizePlanningAttempt(
    await request<unknown>(planningPath(runId), {
      method: 'POST',
      body: JSON.stringify({
        phase: options.phase,
        expected_plan_revision: options.expectedPlanRevision,
        idempotency_key: options.idempotencyKey,
        assistant_id: options.assistantId,
        model: options.model,
      }),
    })
  );
}

/** Applying a candidate does not confirm it or start development. */
export async function applyProductFactoryPlanning(
  runId: string,
  attemptId: string,
  revision: number
): Promise<ProductFactoryRun> {
  return normalizeRun(
    await request<ApiRun>(`${planningPath(runId)}/${encodeURIComponent(attemptId)}/apply`, {
      method: 'POST',
      body: JSON.stringify({ expected_plan_revision: revision }),
    })
  );
}

export async function cancelProductFactoryPlanning(runId: string, attemptId: string): Promise<PlanningAttempt> {
  return normalizePlanningAttempt(
    await request<unknown>(`${planningPath(runId)}/${encodeURIComponent(attemptId)}/cancel`, {
      method: 'POST',
      body: JSON.stringify({}),
    })
  );
}

type ApiExecution = {
  state: FactoryExecution['state'];
  team_id: string;
  task_id: string;
  message_id?: string | null;
  team_run_id?: string | null;
  requested_at: number;
  updated_at: number;
};

type ApiReviewTask = {
  id: string;
  team_id: string;
  subject: string;
  description?: string | null;
  status: string;
  owner?: string | null;
  blocked_by: string[];
  blocks: string[];
  created_at: number;
  updated_at: number;
};

type ApiReviewUsage = {
  id: string;
  task_id?: string | null;
  agent_id?: string | null;
  model?: string | null;
  input_tokens: number;
  output_tokens: number;
  cost_est?: number | null;
  cached_read_tokens?: number | null;
  cached_write_tokens?: number | null;
  cost_source?: string | null;
  pricing_snapshot?: string | null;
  cost_unknown_reason?: string | null;
  conversation_id: string;
  turn_id: string;
  attempt_id?: string | null;
  created_at: number;
};

type ApiReviewUsageSummary = {
  unassigned_usage?: ApiReviewUsage[];
  team_id: string;
  input_tokens: number;
  output_tokens: number;
  cost_est?: number | null;
  cost_unknown: boolean;
  budget_limit_usd?: number | null;
  budget_remaining_usd?: number | null;
  budget_exceeded: boolean;
  tasks: Array<{
    task_id: string;
    input_tokens: number;
    output_tokens: number;
    cost_est?: number | null;
    turn_count: number;
  }>;
};

type ApiReview = {
  run: ApiRun;
  task: ApiReviewTask;
  workspace: { task_id: string; path: string };
  usage: ApiReviewUsage[];
  usage_summary: ApiReviewUsageSummary;
  acceptance_criteria: string[];
  next_task?: { id: string; title: string; acceptance_criteria: string[] } | null;
  execution?: ApiExecution | null;
};

const parseExecution = (value: unknown): FactoryExecution | undefined => {
  if (value == null) return undefined;
  if (typeof value !== 'object') throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
  const raw = value as ApiExecution;
  if (
    !['pending', 'enqueued', 'uncertain'].includes(raw.state) ||
    typeof raw.team_id !== 'string' ||
    !raw.team_id ||
    typeof raw.task_id !== 'string' ||
    !raw.task_id ||
    !Number.isFinite(raw.requested_at) ||
    !Number.isFinite(raw.updated_at) ||
    (raw.message_id != null && (typeof raw.message_id !== 'string' || !raw.message_id)) ||
    (raw.team_run_id != null && (typeof raw.team_run_id !== 'string' || !raw.team_run_id)) ||
    (raw.state === 'enqueued' && (!raw.message_id || !raw.team_run_id))
  ) {
    throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
  }
  return {
    state: raw.state,
    teamId: raw.team_id,
    taskId: raw.task_id,
    messageId: raw.message_id ?? undefined,
    teamRunId: raw.team_run_id ?? undefined,
    requestedAt: raw.requested_at,
    updatedAt: raw.updated_at,
  };
};

function normalizeUsage(row: ApiReviewUsage): FactoryTaskUsage {
  return {
    id: row.id,
    taskId: row.task_id ?? undefined,
    agentId: row.agent_id ?? undefined,
    model: row.model ?? undefined,
    inputTokens: row.input_tokens,
    outputTokens: row.output_tokens,
    costEst: row.cost_est ?? undefined,
    cachedReadTokens: row.cached_read_tokens ?? undefined,
    cachedWriteTokens: row.cached_write_tokens ?? undefined,
    costSource: row.cost_source ?? undefined,
    pricingSnapshot: row.pricing_snapshot ?? undefined,
    costUnknownReason: row.cost_unknown_reason ?? undefined,
    conversationId: row.conversation_id,
    turnId: row.turn_id,
    attemptId: row.attempt_id ?? undefined,
    createdAt: row.created_at,
  };
}

const normalizeReview = (value: ApiReview): ProductFactoryReview => {
  if (!value || typeof value !== 'object' || !value.run || !value.task || !value.workspace) {
    throw new Error('PRODUCT_FACTORY_INVALID_REVIEW_RESPONSE');
  }
  const task = value.task;
  if (
    typeof task.id !== 'string' ||
    typeof task.team_id !== 'string' ||
    typeof task.subject !== 'string' ||
    typeof task.status !== 'string' ||
    !Array.isArray(task.blocked_by) ||
    !Array.isArray(task.blocks)
  ) {
    throw new Error('PRODUCT_FACTORY_INVALID_REVIEW_RESPONSE');
  }
  const usage = (value.usage ?? []).map(normalizeUsage);
  const summary = value.usage_summary;
  if (!summary || typeof summary.team_id !== 'string' || !Array.isArray(summary.tasks)) {
    throw new Error('PRODUCT_FACTORY_INVALID_REVIEW_RESPONSE');
  }
  const normalizedTask: FactoryTask = {
    id: task.id,
    teamId: task.team_id,
    subject: task.subject,
    description: task.description ?? undefined,
    status: task.status,
    owner: task.owner ?? undefined,
    blockedBy: task.blocked_by,
    blocks: task.blocks,
    createdAt: task.created_at,
    updatedAt: task.updated_at,
  };
  return {
    run: normalizeRun(value.run),
    task: normalizedTask,
    workspace: { taskId: value.workspace.task_id, path: value.workspace.path },
    usage,
    usageSummary: {
      teamId: summary.team_id,
      inputTokens: summary.input_tokens,
      outputTokens: summary.output_tokens,
      costEst: summary.cost_est ?? undefined,
      costUnknown: summary.cost_unknown === true,
      budgetLimitUsd: summary.budget_limit_usd ?? undefined,
      budgetRemainingUsd: summary.budget_remaining_usd ?? undefined,
      budgetExceeded: summary.budget_exceeded === true,
      unassignedUsage: (summary.unassigned_usage ?? []).map(normalizeUsage),
      tasks: summary.tasks.map((usageTask) => ({
        taskId: usageTask.task_id,
        inputTokens: usageTask.input_tokens,
        outputTokens: usageTask.output_tokens,
        costEst: usageTask.cost_est ?? undefined,
        turnCount: usageTask.turn_count,
        attemptCount: usageTask.turn_count,
      })),
    },
    acceptanceCriteria: Array.isArray(value.acceptance_criteria) ? value.acceptance_criteria : [],
    nextTask: value.next_task
      ? {
          id: value.next_task.id,
          title: value.next_task.title,
          acceptanceCriteria: value.next_task.acceptance_criteria,
        }
      : undefined,
    execution: parseExecution(value.execution),
  };
};

export async function getProductFactoryExecution(runId: string): Promise<FactoryExecution | undefined> {
  const body = await request<{ execution: unknown }>(
    `/api/product-factory/runs/${encodeURIComponent(runId)}/execution`,
    { cache: 'no-store' }
  );
  if (!Object.hasOwn(body, 'execution')) throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
  return parseExecution(body.execution);
}

export async function startProductFactoryRun(runId: string, revision: number): Promise<FactoryExecution> {
  const body = await request<{ execution: unknown }>(`/api/product-factory/runs/${encodeURIComponent(runId)}/start`, {
    method: 'POST',
    body: JSON.stringify({ expected_revision: revision }),
  });
  const execution = parseExecution(body.execution);
  if (!execution) throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
  return execution;
}

export async function getProductFactoryReview(runId: string): Promise<ProductFactoryReview> {
  return normalizeReview(
    await request<ApiReview>('/api/product-factory/runs/' + encodeURIComponent(runId) + '/review', {
      cache: 'no-store',
    })
  );
}

export async function getProductFactoryDelivery(runId: string): Promise<ProductFactoryDelivery> {
  const value = await request<{
    run: ApiRun;
    ready: boolean;
    workspace_path: string;
    task_counts: {
      total: number;
      pending: number;
      in_progress: number;
      in_review: number;
      completed: number;
      failed: number;
    };
    usage_summary: ApiReviewUsageSummary;
    checks: Array<{ code: string; passed: boolean; detail: string }>;
  }>('/api/product-factory/runs/' + encodeURIComponent(runId) + '/delivery', { cache: 'no-store' });
  const summary = value.usage_summary;
  return {
    run: normalizeRun(value.run),
    ready: value.ready === true,
    workspacePath: value.workspace_path,
    taskCounts: {
      total: value.task_counts.total,
      pending: value.task_counts.pending,
      inProgress: value.task_counts.in_progress,
      inReview: value.task_counts.in_review,
      completed: value.task_counts.completed,
      failed: value.task_counts.failed,
    },
    usageSummary: {
      teamId: summary.team_id,
      inputTokens: summary.input_tokens,
      outputTokens: summary.output_tokens,
      costEst: summary.cost_est ?? undefined,
      costUnknown: summary.cost_unknown === true,
      budgetLimitUsd: summary.budget_limit_usd ?? undefined,
      budgetRemainingUsd: summary.budget_remaining_usd ?? undefined,
      budgetExceeded: summary.budget_exceeded === true,
      unassignedUsage: (summary.unassigned_usage ?? []).map(normalizeUsage),
      tasks: summary.tasks.map((task) => ({
        taskId: task.task_id,
        inputTokens: task.input_tokens,
        outputTokens: task.output_tokens,
        costEst: task.cost_est ?? undefined,
        turnCount: task.turn_count,
        attemptCount: task.turn_count,
      })),
    },
    checks: Array.isArray(value.checks) ? value.checks : [],
  };
}

/** Re-estimate only unknown, fully captured usage using configured rates. */
export async function repriceProductFactoryCosts(runId: string): Promise<{
  updatedCount: number;
  skippedCount: number;
  delivery: ProductFactoryDelivery;
}> {
  const result = await request<{ updated_count: number; skipped_count: number }>(
    `/api/product-factory/runs/${encodeURIComponent(runId)}/cost-report/reprice`,
    { method: 'POST', body: '{}' }
  );
  return {
    updatedCount: result.updated_count,
    skippedCount: result.skipped_count,
    delivery: await getProductFactoryDelivery(runId),
  };
}

export async function getProductFactoryCostReport(runId: string): Promise<ProductFactoryCostReport> {
  const report = await request<{
    run: ApiRun;
    usage?: ApiReviewUsage[];
    usage_summary: ApiReviewUsageSummary;
    task_counts: {
      total: number;
      pending: number;
      in_progress: number;
      in_review: number;
      completed: number;
      failed: number;
    };
    generated_at: number;
    planning_usage?: Array<{
      planning_attempt_id: string;
      phase: PlanningPhase;
      state: PlanningState;
      cost_unknown: boolean;
      usage: ApiReviewUsage[];
    }>;
  }>('/api/product-factory/runs/' + encodeURIComponent(runId) + '/cost-report', { cache: 'no-store' });
  const summary = report.usage_summary;
  return {
    run: normalizeRun(report.run),
    usage: (report.usage ?? []).map(normalizeUsage),
    generatedAt: report.generated_at,
    planningUsage: report.planning_usage?.map((item) => ({
      planningAttemptId: item.planning_attempt_id,
      phase: item.phase,
      state: item.state,
      costUnknown: item.cost_unknown,
      usage: item.usage.map(normalizeUsage),
    })),
    taskCounts: {
      total: report.task_counts.total,
      pending: report.task_counts.pending,
      inProgress: report.task_counts.in_progress,
      inReview: report.task_counts.in_review,
      completed: report.task_counts.completed,
      failed: report.task_counts.failed,
    },
    usageSummary: {
      teamId: summary.team_id,
      inputTokens: summary.input_tokens,
      outputTokens: summary.output_tokens,
      costEst: summary.cost_est ?? undefined,
      costUnknown: summary.cost_unknown === true,
      budgetLimitUsd: summary.budget_limit_usd ?? undefined,
      budgetRemainingUsd: summary.budget_remaining_usd ?? undefined,
      budgetExceeded: summary.budget_exceeded === true,
      unassignedUsage: (summary.unassigned_usage ?? []).map(normalizeUsage),
      tasks: summary.tasks.map((task) => ({
        taskId: task.task_id,
        inputTokens: task.input_tokens,
        outputTokens: task.output_tokens,
        costEst: task.cost_est ?? undefined,
        turnCount: task.turn_count,
        attemptCount: task.turn_count,
      })),
    },
  };
}

export async function reviewProductFactoryTask(
  runId: string,
  decision: 'approve' | 'request_changes',
  feedback?: string
): Promise<ProductFactoryReview> {
  return normalizeReview(
    await request<ApiReview>('/api/product-factory/runs/' + encodeURIComponent(runId) + '/review', {
      method: 'POST',
      body: JSON.stringify({ decision, ...(feedback?.trim() ? { feedback: feedback.trim() } : {}) }),
    })
  );
}

export async function continueProductFactoryRun(runId: string): Promise<FactoryExecution> {
  const body = await request<{ execution: unknown }>(
    '/api/product-factory/runs/' + encodeURIComponent(runId) + '/continue',
    { method: 'POST' }
  );
  const execution = parseExecution(body.execution);
  if (!execution) throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
  return execution;
}

/** Re-read the persisted snapshot without rejecting invalid fields that the preview must explain. */
export async function getProductFactoryRunForPreview(runId: string): Promise<ProductFactoryRun> {
  return normalizeRun(
    await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}`, { cache: 'no-store' }),
    true
  );
}

/** Metadata-only POST: does not create a directory, enumerate contents, or write a file. */
export async function checkProductFactoryWorkspace(path: string): Promise<HandoffIssueCode | undefined> {
  const invalidPath = workspacePathIssue(path);
  if (invalidPath) return invalidPath;
  try {
    const metadata = await request<{ is_directory?: boolean }>('/api/fs/metadata', {
      method: 'POST',
      body: JSON.stringify({ path, workspace: path }),
    });
    return metadata.is_directory === true ? undefined : 'workspaceNotDirectory';
  } catch (error) {
    return error instanceof ProductFactoryHttpError && error.status === 404
      ? 'workspaceNotFound'
      : 'workspaceUnavailable';
  }
}

export async function createProductFactoryRun(draft: ProductIdeaDraft): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>('/api/product-factory/runs', {
    method: 'POST',
    body: JSON.stringify({
      name: draft.name,
      idea: draft.idea,
      target_user: draft.targetUser || undefined,
      problem: draft.problem || undefined,
      expected_output: draft.expectedOutput || undefined,
      workspace_path: draft.workspacePath || undefined,
      budget_usd: draft.budgetUsd,
    }),
  });
  return normalizeRun(run);
}

export async function saveProductFactoryInterview(
  runId: string,
  interview: InterviewArtifact
): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/interview`, {
    method: 'PUT',
    body: JSON.stringify({
      interview:
        interview.version === 2
          ? {
              version: 2,
              generated_by: interview.generatedBy,
              confirmed: interview.confirmed,
              summary: interview.summary,
              questions: interview.questions.map((question) => ({
                id: question.id,
                question: question.prompt,
                reason: question.reason ?? '',
                answer: question.answer,
                not_sure: question.notSure,
              })),
              updated_at: interview.updatedAt,
            }
          : interview,
    }),
  });
  return normalizeRun(run);
}

export async function confirmProductFactoryInterview(runId: string): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/interview/confirm`, {
    method: 'POST',
  });
  return normalizeRun(run);
}

export async function saveProductFactoryBlueprint(
  runId: string,
  blueprint: BlueprintArtifact
): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/blueprint`, {
    method: 'PUT',
    body: JSON.stringify({
      blueprint:
        blueprint.version === 2
          ? {
              version: 2,
              generated_by: blueprint.generatedBy,
              confirmed: blueprint.confirmed,
              sections: blueprint.sections,
              risks: blueprint.risks,
              open_questions: blueprint.openQuestions,
              updated_at: blueprint.updatedAt,
              requirements: blueprint.requirements?.map((requirement) => ({
                id: requirement.id,
                title: requirement.title,
                acceptance_criteria: requirement.acceptanceCriteria,
              })),
            }
          : blueprint,
    }),
  });
  return normalizeRun(run);
}

export async function confirmProductFactoryBlueprint(runId: string): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/blueprint/confirm`, {
    method: 'POST',
  });
  return normalizeRun(run);
}

export async function generateProductFactoryTaskDraft(runId: string): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/task-draft/generate`, {
    method: 'POST',
  });
  return normalizeRun(run);
}

export async function saveProductFactoryTaskDraft(runId: string, draft: TaskDraftArtifact): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/task-draft`, {
    method: 'PUT',
    body: JSON.stringify({
      expected_revision: draft.revision,
      tasks: draft.tasks.map((task) => ({
        id: task.id,
        title: task.title,
        description: task.description,
        type: task.type,
        blocked_by: task.blockedBy,
        acceptance_criteria: task.acceptanceCriteria,
        suggested_role: task.suggestedRole,
        effort: task.effort,
        ...(draft.version === 2 ? { execution_scope: task.executionScope, requirement_ids: task.requirementIds } : {}),
      })),
    }),
  });
  return normalizeRun(run);
}

export async function confirmProductFactoryTaskDraft(runId: string, revision: number): Promise<ProductFactoryRun> {
  const run = await request<ApiRun>(`/api/product-factory/runs/${encodeURIComponent(runId)}/task-draft/confirm`, {
    method: 'POST',
    body: JSON.stringify({ expected_revision: revision }),
  });
  return normalizeRun(run);
}

/** Submit the confirmed revision to the authoritative, idempotent backend operation. */
export async function handoffProductFactoryRun(
  runId: string,
  revision: number,
  agents: TeamAssistantInput[]
): Promise<ProductFactoryRun> {
  const result = await request<{ run: ApiRun; team_id: string; task_id_map: Record<string, string> }>(
    `/api/product-factory/runs/${encodeURIComponent(runId)}/handoff`,
    {
      method: 'POST',
      body: JSON.stringify({
        expected_revision: revision,
        agents: agents.map((agent) => {
          if (typeof agent.model !== 'string') throw new Error('PRODUCT_FACTORY_MEMBER_MODEL_UNRESOLVED');
          // Preserve an intentionally empty model supplied by the existing resolver.
          return { ...toBackendAssistant(agent), model: agent.model };
        }),
      }),
    }
  );
  if (!result.team_id || result.run?.id !== runId || result.run?.team_id !== result.team_id) {
    throw new Error('PRODUCT_FACTORY_INVALID_HANDOFF_RESPONSE');
  }
  return normalizeRun(result.run);
}
