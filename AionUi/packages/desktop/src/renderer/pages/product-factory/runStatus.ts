import type { FactoryRunStatus } from './types';

const RUN_STATUS_KEYS = {
  draft: 'productFactory.handoff.statuses.draft',
  interviewing: 'productFactory.handoff.statuses.interviewing',
  blueprint_generating: 'productFactory.handoff.statuses.blueprintGenerating',
  blueprint_ready: 'productFactory.handoff.statuses.blueprintReady',
  task_draft_generating: 'productFactory.handoff.statuses.taskDraftGenerating',
  task_draft_ready: 'productFactory.handoff.statuses.taskDraftReady',
  handed_off: 'productFactory.handoff.statuses.handedOff',
  running: 'productFactory.handoff.statuses.running',
  in_review: 'productFactory.handoff.statuses.inReview',
  completed: 'productFactory.handoff.statuses.completed',
  failed: 'productFactory.handoff.statuses.failed',
} as const satisfies Record<FactoryRunStatus, string>;

export const factoryRunStatusKey = (status: FactoryRunStatus) => RUN_STATUS_KEYS[status] ?? 'common.status';
