import type { ProductFactoryRun } from '@/renderer/pages/product-factory/types';

export const apiRun = {
  id: 'run-1',
  name: 'Product',
  idea: 'Plan a useful product',
  target_user: '',
  problem: '',
  expected_output: '',
  workspace_path: '/workspace/product',
  status: 'draft',
  plan_revision: 1,
  created_at: 0,
  updated_at: 0,
};

export const run: ProductFactoryRun = {
  id: apiRun.id,
  name: apiRun.name,
  idea: apiRun.idea,
  targetUser: '',
  problem: '',
  expectedOutput: '',
  workspacePath: apiRun.workspace_path,
  status: 'draft',
  planRevision: 1,
  createdAt: 0,
  updatedAt: 0,
};

export const candidate = {
  version: 2,
  generated_by: 'model',
  confirmed: false,
  summary: '',
  questions: [{ id: 'users', question: 'Who will use it?', reason: 'Define the audience', answer: '' }],
};

export const attempt = {
  id: 'attempt-1',
  run_id: 'run-1',
  phase: 'interview',
  input_plan_revision: 1,
  input_hash: 'hash',
  idempotency_key: 'key-1',
  assistant_id: 'configured-assistant',
  model: '',
  conversation_id: 'planning-conversation',
  app_turn_id: 'app-turn-1',
  state: 'candidate_ready',
  candidate: candidate as Record<string, unknown> | null,
  error_code: null as string | null,
  created_at: 1,
  updated_at: 2,
};

export const costReport = {
  run: apiRun,
  usage: [],
  planning_usage: [],
  generated_at: 2,
  task_counts: { total: 0, pending: 0, in_progress: 0, in_review: 0, completed: 0, failed: 0 },
  usage_summary: {
    team_id: '',
    input_tokens: 0,
    output_tokens: 0,
    cost_est: null,
    cost_unknown: true,
    budget_limit_usd: null,
    budget_remaining_usd: null,
    budget_exceeded: false,
    tasks: [],
  },
};

export const v2Draft = {
  version: 2,
  generated_by: 'model',
  confirmed: false,
  revision: 1,
  updated_at: 1,
  tasks: [
    {
      id: 'build',
      title: 'Build CLI',
      description: 'Implement the command',
      type: 'backend',
      blocked_by: [],
      acceptance_criteria: ['Runs locally'],
      suggested_role: 'developer',
      effort: 'medium',
      execution_scope: 'task_workspace',
      requirement_ids: ['req-1'],
    },
    {
      id: 'verify',
      title: 'Verify command',
      description: 'Check invalid input',
      type: 'test',
      blocked_by: ['build'],
      acceptance_criteria: ['Errors are explicit'],
      suggested_role: 'qa',
      effort: 'low',
      execution_scope: 'task_workspace',
      requirement_ids: ['req-1'],
    },
    {
      id: 'integrate',
      title: 'Integrate CLI',
      description: 'Assemble and restart',
      type: 'test',
      blocked_by: ['verify'],
      acceptance_criteria: ['START.md and restart check'],
      suggested_role: 'qa',
      effort: 'medium',
      execution_scope: 'project_integration',
      requirement_ids: ['req-1'],
    },
  ],
};
