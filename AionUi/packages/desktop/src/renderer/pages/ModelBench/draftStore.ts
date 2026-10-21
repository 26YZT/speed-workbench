import type { BenchModel, BenchRecord, BenchResult } from './types';

export type BenchDraft = {
  version: 1;
  prompt: string;
  selected: string[];
  selectedModels: BenchModel[];
  material?: { name: string; text: string };
  results: BenchResult[];
  snapshot: Pick<BenchRecord, 'id' | 'created' | 'prompt'>;
};

type DraftStorage = Pick<Storage, 'getItem' | 'setItem'>;
const draftKey = (owner: string) => `speed-workbench:model-bench-draft:v1:${encodeURIComponent(owner)}`;
const isModel = (value: unknown): value is BenchModel => {
  if (!value || typeof value !== 'object') return false;
  const model = value as BenchModel;
  return [model.provider_id, model.provider_name, model.model].every((item) => typeof item === 'string');
};
const cleanModel = ({ provider_id, provider_name, model }: BenchModel): BenchModel => ({
  provider_id,
  provider_name,
  model,
});
const cleanResult = (result: BenchResult): BenchResult => ({
  target: cleanModel(result.target),
  text: result.text,
  elapsed: result.elapsed,
  status: result.status === 'running' || result.status === 'waiting' ? 'stopped' : result.status,
  ...(Number.isFinite(result.startedAt) ? { startedAt: result.startedAt } : {}),
  ...(typeof result.error === 'string' ? { error: result.error } : {}),
});
const isResult = (value: unknown): value is BenchResult => {
  if (!value || typeof value !== 'object') return false;
  const result = value as BenchResult;
  return (
    isModel(result.target) &&
    typeof result.text === 'string' &&
    Number.isFinite(result.elapsed) &&
    ['waiting', 'running', 'done', 'error', 'stopped'].includes(result.status)
  );
};

/** Interrupted comparisons are recovered as stopped, never re-issued. */
export function readBenchDraft(owner: string, storage?: DraftStorage): BenchDraft | undefined {
  try {
    const value = (storage ?? localStorage).getItem(draftKey(owner));
    if (!value) return undefined;
    const draft = JSON.parse(value) as BenchDraft;
    if (
      draft.version !== 1 ||
      typeof draft.prompt !== 'string' ||
      !Array.isArray(draft.selected) ||
      !draft.selected.every((key) => typeof key === 'string') ||
      !Array.isArray(draft.results) ||
      !draft.results.every(isResult) ||
      !draft.snapshot ||
      typeof draft.snapshot.id !== 'string' ||
      typeof draft.snapshot.prompt !== 'string' ||
      !Number.isFinite(draft.snapshot.created)
    )
      return undefined;
    if (draft.material && (typeof draft.material.name !== 'string' || typeof draft.material.text !== 'string'))
      return undefined;
    return {
      ...draft,
      selected: draft.selected.slice(0, 4),
      selectedModels: Array.isArray(draft.selectedModels) ? draft.selectedModels.filter(isModel).map(cleanModel) : [],
      results: draft.results.map(cleanResult),
    };
  } catch {
    return undefined;
  }
}

export function writeBenchDraft(owner: string, draft: BenchDraft, storage?: DraftStorage): boolean {
  try {
    (storage ?? localStorage).setItem(
      draftKey(owner),
      JSON.stringify({
        ...draft,
        selectedModels: draft.selectedModels.map(cleanModel),
        results: draft.results.map(cleanResult),
      })
    );
    return true;
  } catch {
    return false;
  }
}
