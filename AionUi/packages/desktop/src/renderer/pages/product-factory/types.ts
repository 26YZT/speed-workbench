/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

export type FactoryRunStatus =
  | 'draft'
  | 'interviewing'
  | 'blueprint_generating'
  | 'blueprint_ready'
  | 'task_draft_generating'
  | 'task_draft_ready'
  | 'handed_off'
  | 'running'
  | 'in_review'
  | 'completed'
  | 'failed';

export type ProductIdeaDraft = {
  name: string;
  idea: string;
  targetUser: string;
  problem: string;
  expectedOutput: string;
  workspacePath: string;
  budgetUsd?: number;
};

/** A durable dispatch receipt; it does not establish Agent or product success. */
export type FactoryExecution = {
  state: 'pending' | 'enqueued' | 'uncertain';
  teamId: string;
  taskId: string;
  messageId?: string;
  teamRunId?: string;
  requestedAt: number;
  updatedAt: number;
};

export type FactoryTask = {
  id: string;
  teamId: string;
  subject: string;
  description?: string;
  status: string;
  owner?: string;
  blockedBy: string[];
  blocks: string[];
  createdAt: number;
  updatedAt: number;
};

export type FactoryTaskUsage = {
  id: string;
  taskId?: string;
  agentId?: string;
  model?: string;
  inputTokens: number;
  outputTokens: number;
  costEst?: number;
  cachedReadTokens?: number;
  cachedWriteTokens?: number;
  costSource?: string;
  pricingSnapshot?: string;
  costUnknownReason?: string;
  conversationId: string;
  turnId: string;
  attemptId?: string;
  createdAt: number;
};

export type FactoryUsageSummary = {
  teamId: string;
  inputTokens: number;
  outputTokens: number;
  costEst?: number;
  costUnknown: boolean;
  budgetLimitUsd?: number;
  budgetRemainingUsd?: number;
  budgetExceeded: boolean;
  unassignedUsage?: FactoryTaskUsage[];
  tasks: Array<{
    taskId: string;
    inputTokens: number;
    outputTokens: number;
    costEst?: number;
    turnCount: number;
    /** Ledger attempts, including proven not_sent markers; not model calls. */
    attemptCount?: number;
  }>;
};

export type FactoryNextTask = {
  id: string;
  title: string;
  acceptanceCriteria: string[];
};

export type ProductFactoryReview = {
  run: ProductFactoryRun;
  task: FactoryTask;
  workspace: { taskId: string; path: string };
  usage: FactoryTaskUsage[];
  usageSummary: FactoryUsageSummary;
  acceptanceCriteria: string[];
  nextTask?: FactoryNextTask;
  execution?: FactoryExecution;
};

export type ProductFactoryDelivery = {
  run: ProductFactoryRun;
  ready: boolean;
  workspacePath: string;
  taskCounts: {
    total: number;
    pending: number;
    inProgress: number;
    inReview: number;
    completed: number;
    failed: number;
  };
  usageSummary: FactoryUsageSummary;
  checks: Array<{ code: string; passed: boolean; detail: string }>;
};

export type ProductFactoryCostReport = {
  run: ProductFactoryRun;
  usage?: FactoryTaskUsage[];
  taskCounts: ProductFactoryDelivery['taskCounts'];
  usageSummary: FactoryUsageSummary;
  generatedAt: number;
  planningUsage?: Array<{
    planningAttemptId: string;
    phase: PlanningPhase;
    state: PlanningState;
    costUnknown: boolean;
    usage: FactoryTaskUsage[];
  }>;
};

export type FactoryRunSummary = {
  id: string;
  name: string;
  status: FactoryRunStatus;
  workspacePath: string;
  budgetUsd?: number;
  spentUsd: number;
  updatedAt: string;
  teamId?: string;
};

export type ProductFactoryRun = ProductIdeaDraft & {
  id: string;
  status: FactoryRunStatus;
  interview?: unknown;
  blueprint?: unknown;
  taskDraft?: unknown;
  teamId?: string;
  createdAt: number;
  updatedAt: number;
  planRevision?: number;
};

export type InterviewQuestion = {
  id: string;
  prompt: string;
  answer: string;
  notSure: boolean;
  reason?: string;
};

export type InterviewArtifact = {
  version: 1 | 2;
  questions: InterviewQuestion[];
  summary: string;
  confirmed: boolean;
  updatedAt: number;
  generatedBy?: 'draft' | 'model';
};

export type BlueprintSection = {
  id: string;
  title: string;
  content: string;
  confirmed: boolean;
};

export type BlueprintArtifact = {
  version: 1 | 2;
  sections: BlueprintSection[];
  risks: string[];
  openQuestions: string[];
  generatedBy: 'draft' | 'model';
  confirmed: boolean;
  updatedAt: number;
  requirements?: BlueprintRequirement[];
};

export type BlueprintRequirement = { id: string; title: string; acceptanceCriteria: string[] };

export type TaskDraftType = 'frontend' | 'backend' | 'data' | 'test';
export type TaskDraftEffort = 'low' | 'medium' | 'high';

export type TaskDraftTask = {
  id: string;
  title: string;
  description: string;
  type: TaskDraftType;
  blockedBy: string[];
  acceptanceCriteria: string[];
  suggestedRole: string;
  effort: TaskDraftEffort;
  executionScope?: 'task_workspace' | 'project_integration';
  requirementIds?: string[];
};

export type TaskDraftArtifact = {
  version: 1 | 2;
  generatedBy: 'draft' | 'model';
  confirmed: boolean;
  revision: number;
  updatedAt: number;
  tasks: TaskDraftTask[];
};

export type PlanningPhase = 'interview' | 'blueprint' | 'task_graph';
export type PlanningState =
  | 'reserved'
  | 'preparing'
  | 'running'
  | 'candidate_ready'
  | 'applied'
  | 'failed'
  | 'cancelled'
  | 'uncertain';

export type PlanningAttempt = {
  id: string;
  runId: string;
  phase: PlanningPhase;
  inputPlanRevision: number;
  inputHash: string;
  idempotencyKey: string;
  assistantId: string;
  model: string;
  conversationId?: string;
  appTurnId?: string;
  state: PlanningState;
  candidate?: Record<string, unknown>;
  errorCode?: string;
  createdAt: number;
  updatedAt: number;
};

export type CodeManifest = {
  version: 1;
  scope: 'code_only';
  files: Array<{ path: string; sha256: string; sizeBytes: number }>;
  totalBytes: number;
};
export type ProductVersion = {
  id: string;
  productId: string;
  runId: string;
  parentRunId?: string;
  parentVersionId?: string;
  versionNo: number;
  state: 'copying' | 'working' | 'sealed' | 'failed';
  snapshotPath?: string;
  manifest?: CodeManifest;
  changeRequest?: string;
  errorCode?: string;
  createdAt: number;
  updatedAt: number;
  sealedAt?: number;
};
export type VersionProduct = {
  id: string;
  name: string;
  activeVersionId?: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
};
export type VersionOperation = {
  id: string;
  kind: 'snapshot' | 'iterate';
  state: 'reserved' | 'copying' | 'complete' | 'failed';
  versionId: string;
  idempotencyKey: string;
  errorCode?: string;
  createdAt: number;
  updatedAt: number;
};
export type ProductVersions = {
  product?: VersionProduct;
  currentVersionId?: string;
  versions: ProductVersion[];
  operations: VersionOperation[];
};
export type SourceManifest = {
  runId: string;
  planRevision: number;
  manifest: CodeManifest;
  excludedCount: number;
  warnings: string[];
};
export type VersionReceipt = { product: VersionProduct; version: ProductVersion; operation: VersionOperation };
export type SnapshotVersionInput = {
  expected_plan_revision: number;
  idempotency_key: string;
  files: Array<{ path: string; sha256: string }>;
};
export type IterateVersionInput = {
  source_version_id: string;
  expected_product_revision: number;
  idempotency_key: string;
  change_request: string;
  budget_usd?: number;
};
export type PendingVersionWrite =
  | { kind: 'snapshot'; input: SnapshotVersionInput }
  | { kind: 'iterate'; input: IterateVersionInput };
