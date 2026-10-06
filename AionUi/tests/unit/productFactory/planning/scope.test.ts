import { describe, expect, it } from 'vitest';
import {
  compareHandoffSnapshot,
  inspectHandoffDraft,
  normalizePreviewDraft,
} from '@/renderer/pages/product-factory/handoffPreview';
import type { ProductFactoryRun } from '@/renderer/pages/product-factory/types';
import { run, v2Draft } from './fixtures';

const completeRun = (draft = v2Draft): ProductFactoryRun => ({
  ...run,
  status: 'task_draft_ready',
  blueprint: {
    confirmed: true,
    requirements: [{ id: 'req-1', title: 'CLI workflow', acceptance_criteria: ['Works'] }],
  },
  taskDraft: normalizePreviewDraft({ ...draft, confirmed: true }),
});

describe('explicit integration and requirement preflight', () => {
  it('accepts model v2 with a single terminal integration and transitive dependencies', () => {
    const ready = completeRun();
    expect(compareHandoffSnapshot(ready, ready)).toEqual([]);
  });

  it('blocks a second root integration card even if both cards are test tasks', () => {
    const draft = {
      ...v2Draft,
      tasks: v2Draft.tasks.map((task, index) =>
        index === 1 ? { ...task, execution_scope: 'project_integration' } : task
      ),
    };
    expect(inspectHandoffDraft(completeRun(draft).taskDraft).issues).toContainEqual(
      expect.objectContaining({ code: 'invalidIntegration' })
    );
  });

  it('blocks a graph whose root integration can run before an artifact task', () => {
    const draft = {
      ...v2Draft,
      tasks: v2Draft.tasks.map((task, index) =>
        index === 2 ? Object.assign({}, task, { blocked_by: ['build'] }) : task
      ),
    };
    expect(inspectHandoffDraft(completeRun(draft).taskDraft).issues).toContainEqual(
      expect.objectContaining({ code: 'invalidIntegration' })
    );
  });

  it('blocks unknown requirement links and requirements omitted by the graph', () => {
    const ready = completeRun();
    ready.blueprint = { requirements: [{ id: 'req-2', title: 'Other requirement' }] };
    expect(compareHandoffSnapshot(ready, ready)).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ code: 'unknownRequirement', dependency: 'req-1' }),
        expect.objectContaining({ code: 'uncoveredRequirement', dependency: 'req-2' }),
      ])
    );
  });
});
