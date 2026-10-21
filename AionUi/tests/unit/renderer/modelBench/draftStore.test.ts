import { beforeEach, describe, expect, it } from 'vitest';
import { readBenchDraft, writeBenchDraft, type BenchDraft } from '@/renderer/pages/ModelBench/draftStore';

class MemoryStorage {
  values = new Map<string, string>();
  fail = false;
  getItem(key: string) {
    return this.values.get(key) ?? null;
  }
  setItem(key: string, value: string) {
    if (this.fail) throw new Error('quota');
    this.values.set(key, value);
  }
}
const draft: BenchDraft = {
  version: 1,
  prompt: 'Compare this exact input\n',
  selected: ['model'],
  selectedModels: [{ provider_id: 'p', provider_name: 'Provider', model: 'm' }],
  material: { name: 'notes.md', text: 'Original material' },
  snapshot: { id: 'attempt', created: 1, prompt: 'Original request' },
  results: [
    {
      target: { provider_id: 'p', provider_name: 'Provider', model: 'm' },
      text: 'Partial response',
      status: 'running',
      elapsed: 12,
    },
  ],
};

describe('comparison draft recovery', () => {
  let storage: MemoryStorage;
  beforeEach(() => {
    storage = new MemoryStorage();
  });
  it('restores input, material and partial answers while stopping interrupted requests', () => {
    expect(writeBenchDraft('owner', draft, storage)).toBe(true);
    const restored = readBenchDraft('owner', storage);
    expect(restored?.prompt).toBe(draft.prompt);
    expect(restored?.material).toEqual(draft.material);
    expect(restored?.results[0]).toMatchObject({ text: 'Partial response', status: 'stopped' });
  });
  it('keeps different user drafts isolated', () => {
    writeBenchDraft('owner-A', draft, storage);
    expect(readBenchDraft('owner-B', storage)).toBeUndefined();
  });
  it('reports failed writes and preserves the last recoverable draft', () => {
    writeBenchDraft('owner', draft, storage);
    storage.fail = true;
    expect(writeBenchDraft('owner', { ...draft, prompt: 'Newer input' }, storage)).toBe(false);
    expect(readBenchDraft('owner', storage)?.prompt).toBe(draft.prompt);
  });
  it('does not crash or remove an unreadable stored record', () => {
    storage.values.set('speed-workbench:model-bench-draft:v1:owner', '{broken');
    expect(readBenchDraft('owner', storage)).toBeUndefined();
    expect(storage.values.get('speed-workbench:model-bench-draft:v1:owner')).toBe('{broken');
  });
  it('handles denied access to the storage object itself', () => {
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');
    Object.defineProperty(globalThis, 'localStorage', {
      configurable: true,
      get() {
        throw new Error('Storage access denied');
      },
    });
    try {
      expect(readBenchDraft('owner')).toBeUndefined();
      expect(writeBenchDraft('owner', draft)).toBe(false);
    } finally {
      if (descriptor) Object.defineProperty(globalThis, 'localStorage', descriptor);
      else Reflect.deleteProperty(globalThis, 'localStorage');
    }
  });
  it('persists an explicitly cleared input instead of reviving an older prompt', () => {
    writeBenchDraft('owner', draft, storage);
    writeBenchDraft('owner', { ...draft, prompt: '', material: undefined, results: [], selected: [] }, storage);
    expect(readBenchDraft('owner', storage)?.prompt).toBe('');
  });
});
