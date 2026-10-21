import type { ProductFactoryRun } from './types';

export type EditableArtifacts = Pick<ProductFactoryRun, 'interview' | 'blueprint' | 'taskDraft'>;
export type ArtifactRecovery = { version: 1; runId: string; baseline: EditableArtifacts; edited: EditableArtifacts };
const key = (runId: string) => `product-factory:artifact-recovery:v1:${encodeURIComponent(runId)}`;
export const editableArtifacts = (run: ProductFactoryRun): EditableArtifacts => ({
  interview: run.interview,
  blueprint: run.blueprint,
  taskDraft: run.taskDraft,
});
const canonical = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object')
    return Object.fromEntries(
      Object.entries(value)
        .filter(([, v]) => v !== undefined)
        .toSorted(([a], [b]) => a.localeCompare(b))
        .map(([k, v]) => [k, canonical(v)])
    );
  return value;
};
export const artifactsEqual = (a: EditableArtifacts, b: EditableArtifacts): boolean =>
  JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
const content = (artifacts: EditableArtifacts): EditableArtifacts =>
  Object.fromEntries(
    Object.entries(artifacts).map(([name, value]) => [
      name,
      value && typeof value === 'object'
        ? Object.fromEntries(Object.entries(value).filter(([k]) => !['revision', 'updatedAt', 'confirmed'].includes(k)))
        : value,
    ])
  ) as EditableArtifacts;
const validArtifacts = (value: unknown): value is EditableArtifacts => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const artifacts = value as EditableArtifacts;
  if (
    artifacts.interview &&
    (typeof artifacts.interview !== 'object' ||
      !Array.isArray((artifacts.interview as Record<string, unknown>).questions))
  )
    return false;
  if (artifacts.blueprint && typeof artifacts.blueprint !== 'object') return false;
  if (
    artifacts.taskDraft &&
    (typeof artifacts.taskDraft !== 'object' ||
      !Array.isArray((artifacts.taskDraft as Record<string, unknown>).tasks) ||
      ![1, 2].includes((artifacts.taskDraft as Record<string, unknown>).version as number))
  )
    return false;
  return true;
};

export function readArtifactRecovery(runId: string): ArtifactRecovery | undefined {
  try {
    const raw = localStorage.getItem(key(runId));
    if (!raw) return undefined;
    const value = JSON.parse(raw) as ArtifactRecovery;
    return value.version === 1 &&
      value.runId === runId &&
      validArtifacts(value.baseline) &&
      validArtifacts(value.edited)
      ? {
          version: 1,
          runId,
          baseline: {
            interview: value.baseline.interview,
            blueprint: value.baseline.blueprint,
            taskDraft: value.baseline.taskDraft,
          },
          edited: {
            interview: value.edited.interview,
            blueprint: value.edited.blueprint,
            taskDraft: value.edited.taskDraft,
          },
        }
      : undefined;
  } catch {
    return undefined;
  }
}
export function writeArtifactRecovery(runId: string, baseline: EditableArtifacts, edited: EditableArtifacts): boolean {
  try {
    localStorage.setItem(key(runId), JSON.stringify({ version: 1, runId, baseline, edited }));
    return true;
  } catch {
    return false;
  }
}
export function clearArtifactRecovery(runId: string): void {
  try {
    localStorage.removeItem(key(runId));
  } catch {
    /* The server save remains authoritative. */
  }
}
export function resolveArtifactRecovery(
  run: ProductFactoryRun,
  recovery: ArtifactRecovery
): 'saved' | 'restore' | 'conflict' {
  const server = editableArtifacts(run);
  if (artifactsEqual(content(server), content(recovery.edited))) return 'saved';
  return artifactsEqual(server, recovery.baseline) ? 'restore' : 'conflict';
}
