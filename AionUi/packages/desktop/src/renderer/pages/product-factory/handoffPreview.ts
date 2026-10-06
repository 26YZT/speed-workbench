/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import type { ProductFactoryRun, TaskDraftTask } from './types';

export type HandoffIssueCode =
  | 'missingDraft'
  | 'invalidDraft'
  | 'unconfirmed'
  | 'invalidSource'
  | 'invalidRevision'
  | 'emptyTasks'
  | 'tooManyTasks'
  | 'invalidId'
  | 'duplicateId'
  | 'missingDependency'
  | 'selfDependency'
  | 'duplicateDependency'
  | 'cycle'
  | 'emptyTitle'
  | 'emptyDescription'
  | 'emptyCriteria'
  | 'invalidTask'
  | 'invalidScope'
  | 'missingRequirement'
  | 'unknownRequirement'
  | 'uncoveredRequirement'
  | 'invalidIntegration'
  | 'staleRevision'
  | 'snapshotChanged'
  | 'alreadyHandedOff'
  | 'invalidStatus'
  | 'missingWorkspace'
  | 'invalidWorkspace'
  | 'workspaceNotFound'
  | 'workspaceNotDirectory'
  | 'workspaceUnavailable'
  | 'serverUnavailable'
  | 'versionNotReady';
export type HandoffIssue = { code: HandoffIssueCode; taskId?: string; dependency?: string };

export const asRecord = (value: unknown): Record<string, unknown> =>
  value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
const text = (value: unknown): string => (typeof value === 'string' ? value : '');
const strings = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((v) => typeof v === 'string');

/** Preserve invalid server fields so the read-only preview can explain them without weakening editor parsing. */
export function normalizePreviewDraft(value: unknown): unknown {
  if (value == null) return value;
  const raw = asRecord(value);
  return {
    ...raw,
    generatedBy: raw.generated_by,
    updatedAt: raw.updated_at,
    tasks: Array.isArray(raw.tasks)
      ? raw.tasks.map((taskValue) => {
          const task = asRecord(taskValue);
          return {
            ...task,
            blockedBy: task.blocked_by,
            acceptanceCriteria: task.acceptance_criteria,
            suggestedRole: task.suggested_role,
            executionScope: task.execution_scope,
            requirementIds: task.requirement_ids,
          };
        })
      : raw.tasks,
  };
}

/** UI hints mirror aionui-product-factory/src/task_draft.rs; server checks remain authoritative. */
export function inspectHandoffDraft(draftValue: unknown): { tasks: TaskDraftTask[]; issues: HandoffIssue[] } {
  const issues: HandoffIssue[] = [];
  const add = (code: HandoffIssueCode, taskId?: string, dependency?: string) =>
    issues.push({ code, taskId, dependency });
  if (draftValue == null) return { tasks: [], issues: [{ code: 'missingDraft' }] };
  const draft = asRecord(draftValue);
  if ((draft.version !== 1 && draft.version !== 2) || !Array.isArray(draft.tasks)) add('invalidDraft');
  if (draft.confirmed !== true) add('unconfirmed');
  if ((draft.version === 1 && draft.generatedBy !== 'draft') || (draft.version === 2 && draft.generatedBy !== 'model'))
    add('invalidSource');
  if (!Number.isSafeInteger(draft.revision) || Number(draft.revision) < 1) add('invalidRevision');
  const rawTasks = Array.isArray(draft.tasks) ? draft.tasks : [];
  if (!rawTasks.length) add('emptyTasks');
  if (rawTasks.length > 100) add('tooManyTasks');
  const tasks: TaskDraftTask[] = rawTasks.map((taskValue) => {
    const raw = asRecord(taskValue);
    const id = text(raw.id);
    if (
      !isStringArray(raw.blockedBy) ||
      !isStringArray(raw.acceptanceCriteria) ||
      !['frontend', 'backend', 'data', 'test'].includes(text(raw.type)) ||
      !['low', 'medium', 'high'].includes(text(raw.effort)) ||
      typeof raw.suggestedRole !== 'string'
    )
      add('invalidTask', id);
    const task: TaskDraftTask = {
      id,
      title: text(raw.title),
      description: text(raw.description),
      type: text(raw.type) as TaskDraftTask['type'],
      blockedBy: strings(raw.blockedBy),
      acceptanceCriteria: strings(raw.acceptanceCriteria),
      suggestedRole: text(raw.suggestedRole),
      effort: text(raw.effort) as TaskDraftTask['effort'],
    };
    if (draft.version === 2) {
      task.executionScope = text(raw.executionScope) as TaskDraftTask['executionScope'];
      task.requirementIds = strings(raw.requirementIds);
    }
    return task;
  });
  const ids = new Set<string>();
  for (const task of tasks) {
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(task.id)) add('invalidId', task.id);
    if (ids.has(task.id)) add('duplicateId', task.id);
    ids.add(task.id);
    if (!task.title.trim()) add('emptyTitle', task.id);
    if (!task.description.trim()) add('emptyDescription', task.id);
    if (!task.acceptanceCriteria.some((item) => item.trim())) add('emptyCriteria', task.id);
    if (draft.version === 2) {
      if (!['task_workspace', 'project_integration'].includes(task.executionScope ?? '')) add('invalidScope', task.id);
      if (!task.requirementIds?.length || task.requirementIds.some((id) => !id.trim()))
        add('missingRequirement', task.id);
    }
    const dependencies = new Set<string>();
    for (const dependency of task.blockedBy) {
      if (dependency === task.id) add('selfDependency', task.id, dependency);
      if (dependencies.has(dependency)) add('duplicateDependency', task.id, dependency);
      dependencies.add(dependency);
    }
  }
  for (const task of tasks)
    for (const dependency of task.blockedBy) {
      if (!ids.has(dependency)) add('missingDependency', task.id, dependency);
    }
  // Bound recursion to the backend's accepted task count; oversized drafts are already blocked.
  if (tasks.length <= 100 && ids.size === tasks.length) {
    const graph = new Map(tasks.map((task) => [task.id, task.blockedBy]));
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const hasCycle = (id: string): boolean => {
      if (visiting.has(id)) return true;
      if (visited.has(id)) return false;
      visiting.add(id);
      if (graph.get(id)?.some(hasCycle)) return true;
      visiting.delete(id);
      visited.add(id);
      return false;
    };
    if (tasks.some((task) => hasCycle(task.id))) add('cycle');
    if (draft.version === 2) {
      const integrations = tasks.filter((task) => task.executionScope === 'project_integration');
      if (integrations.length !== 1 || tasks.length < 2) add('invalidIntegration');
      else {
        const integration = integrations[0];
        const covered = new Set<string>();
        const visitDependencies = (id: string) => {
          if (covered.has(id)) return;
          covered.add(id);
          for (const dependency of graph.get(id) ?? []) visitDependencies(dependency);
        };
        for (const dependency of integration.blockedBy) visitDependencies(dependency);
        if (
          integration.type !== 'test' ||
          tasks.some((task) => task.blockedBy.includes(integration.id)) ||
          tasks.some((task) => task.id !== integration.id && !covered.has(task.id))
        )
          add('invalidIntegration', integration.id);
      }
    }
  }
  return { tasks, issues };
}

export function isHandedOff(run: ProductFactoryRun): boolean {
  return Boolean(run.teamId) || ['handed_off', 'running', 'in_review', 'completed'].includes(run.status);
}

export function workspacePathIssue(path: string): HandoffIssueCode | undefined {
  if (!path.trim()) return 'missingWorkspace';
  if (path.includes('\0') || path !== path.trim() || !/^(\/|[A-Za-z]:[\\/]|\\\\[^\\]+\\[^\\]+)/.test(path))
    return 'invalidWorkspace';
  return undefined;
}

export function compareHandoffSnapshot(local: ProductFactoryRun, server: ProductFactoryRun): HandoffIssue[] {
  const issues = [...inspectHandoffDraft(local.taskDraft).issues, ...inspectHandoffDraft(server.taskDraft).issues];
  if (isHandedOff(local) || isHandedOff(server)) issues.push({ code: 'alreadyHandedOff' });
  if (server.status !== 'task_draft_ready') issues.push({ code: 'invalidStatus' });
  const localDraft = asRecord(local.taskDraft);
  const serverDraft = asRecord(server.taskDraft);
  if (localDraft.revision !== serverDraft.revision) issues.push({ code: 'staleRevision' });
  const localTasks = inspectHandoffDraft(local.taskDraft).tasks;
  const serverTasks = inspectHandoffDraft(server.taskDraft).tasks;
  for (const run of [local, server]) {
    if (asRecord(run.taskDraft).version !== 2) continue;
    const requirements = asRecord(run.blueprint).requirements;
    const requirementIds = new Set(
      Array.isArray(requirements) ? requirements.map((value) => text(asRecord(value).id)).filter(Boolean) : []
    );
    const covered = new Set<string>();
    for (const task of inspectHandoffDraft(run.taskDraft).tasks) {
      for (const id of task.requirementIds ?? []) {
        covered.add(id);
        if (!requirementIds.has(id)) issues.push({ code: 'unknownRequirement', taskId: task.id, dependency: id });
      }
    }
    for (const id of requirementIds)
      if (!covered.has(id)) issues.push({ code: 'uncoveredRequirement', dependency: id });
  }
  if (
    local.id !== server.id ||
    local.workspacePath !== server.workspacePath ||
    localDraft.confirmed !== serverDraft.confirmed ||
    local.planRevision !== server.planRevision ||
    JSON.stringify(localTasks) !== JSON.stringify(serverTasks)
  ) {
    issues.push({ code: 'snapshotChanged' });
  }
  return issues.filter(
    (issue, index) => issues.findIndex((item) => JSON.stringify(item) === JSON.stringify(issue)) === index
  );
}
