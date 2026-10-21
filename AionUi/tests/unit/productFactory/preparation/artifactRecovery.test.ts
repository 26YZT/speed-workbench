import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  clearArtifactRecovery,
  editableArtifacts,
  readArtifactRecovery,
  resolveArtifactRecovery,
  writeArtifactRecovery,
} from '@/renderer/pages/product-factory/artifactRecovery';
import type { ProductFactoryRun, TaskDraftArtifact } from '@/renderer/pages/product-factory/types';

const artifact: TaskDraftArtifact = {
  version: 1,
  generatedBy: 'draft',
  revision: 1,
  confirmed: false,
  updatedAt: 1,
  tasks: [],
};
const run = { id: 'run-a', taskDraft: artifact } as ProductFactoryRun;
const edited = { taskDraft: { ...artifact, tasks: [{ id: 'one', title: 'Keep this edit' }] } };
describe('per-product artifact recovery', () => {
  afterEach(() => vi.unstubAllGlobals());
  beforeEach(() => {
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    });
  });
  it('recovers edits only when the server baseline still matches', () => {
    writeArtifactRecovery(run.id, editableArtifacts(run), edited);
    const recovery = readArtifactRecovery(run.id)!;
    expect(resolveArtifactRecovery(run, recovery)).toBe('restore');
    expect((recovery.edited.taskDraft as TaskDraftArtifact).tasks[0].title).toBe('Keep this edit');
    expect(readArtifactRecovery('another-product')).toBeUndefined();
  });
  it('keeps the draft separate when the server changed', () => {
    writeArtifactRecovery(run.id, editableArtifacts(run), edited);
    expect(
      resolveArtifactRecovery({ ...run, taskDraft: { ...artifact, revision: 2 } }, readArtifactRecovery(run.id)!)
    ).toBe('conflict');
    expect(readArtifactRecovery(run.id)).toBeDefined();
  });
  it('recognizes acknowledged edits despite a new server revision and timestamp', () => {
    writeArtifactRecovery(run.id, editableArtifacts(run), edited);
    expect(
      resolveArtifactRecovery(
        { ...run, taskDraft: { ...edited.taskDraft, revision: 2, updatedAt: 3, confirmed: true } },
        readArtifactRecovery(run.id)!
      )
    ).toBe('saved');
    clearArtifactRecovery(run.id);
    expect(readArtifactRecovery(run.id)).toBeUndefined();
  });
  it('reports storage failure without claiming the edits were saved', () => {
    vi.spyOn(localStorage, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    expect(writeArtifactRecovery(run.id, editableArtifacts(run), edited)).toBe(false);
  });
  it('preserves malformed recovery data without applying it to a product', () => {
    localStorage.setItem('product-factory:artifact-recovery:v1:run-a', '{invalid');
    expect(readArtifactRecovery(run.id)).toBeUndefined();
    expect(localStorage.getItem('product-factory:artifact-recovery:v1:run-a')).toBe('{invalid');
  });
  it('never restores cached lifecycle fields over the server-owned product state', () => {
    localStorage.setItem(
      'product-factory:artifact-recovery:v1:run-a',
      JSON.stringify({
        version: 1,
        runId: run.id,
        baseline: editableArtifacts(run),
        edited: { ...edited, status: 'completed', teamId: 'other-team' },
      })
    );
    const recovery = readArtifactRecovery(run.id)!;
    expect(recovery.edited).not.toHaveProperty('status');
    expect(recovery.edited).not.toHaveProperty('teamId');
  });
});
