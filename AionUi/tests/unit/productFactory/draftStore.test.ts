/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { beforeEach, describe, expect, it } from 'vitest';
import {
  EMPTY_PRODUCT_IDEA_DRAFT,
  clearProductIdeaDraft,
  loadProductIdeaDraft,
  saveProductIdeaDraft,
  type ProductIdeaDraftStorage,
} from '@/renderer/pages/product-factory/draftStore';

class MemoryStorage implements ProductIdeaDraftStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) ?? null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

describe('product idea draft storage', () => {
  let storage: MemoryStorage;

  beforeEach(() => {
    storage = new MemoryStorage();
  });

  it('returns a fresh empty draft when no saved value exists', () => {
    const draft = loadProductIdeaDraft(storage);

    expect(draft).toEqual(EMPTY_PRODUCT_IDEA_DRAFT);
    expect(draft).not.toBe(EMPTY_PRODUCT_IDEA_DRAFT);
  });

  it('restores a valid versioned draft', () => {
    storage.setItem(
      'product-factory:idea-draft',
      JSON.stringify({
        version: 1,
        draft: {
          name: 'Research Copilot',
          idea: 'Turn interview notes into a research brief',
          targetUser: 'Product managers',
          problem: 'Research synthesis takes too long',
          expectedOutput: 'Desktop prototype',
          workspacePath: '/tmp/research-copilot',
          budgetUsd: 12.5,
        },
      })
    );

    expect(loadProductIdeaDraft(storage)).toEqual({
      name: 'Research Copilot',
      idea: 'Turn interview notes into a research brief',
      targetUser: 'Product managers',
      problem: 'Research synthesis takes too long',
      expectedOutput: 'Desktop prototype',
      workspacePath: '/tmp/research-copilot',
      budgetUsd: 12.5,
    });
  });

  it('ignores malformed JSON instead of crashing the page', () => {
    storage.setItem('product-factory:idea-draft', '{invalid');

    expect(loadProductIdeaDraft(storage)).toEqual(EMPTY_PRODUCT_IDEA_DRAFT);
  });

  it('ignores drafts written by an incompatible storage version', () => {
    storage.setItem(
      'product-factory:idea-draft',
      JSON.stringify({ version: 2, draft: { name: 'Old', idea: 'Old idea' } })
    );

    expect(loadProductIdeaDraft(storage)).toEqual(EMPTY_PRODUCT_IDEA_DRAFT);
  });

  it('normalizes invalid fields and removes a non-positive budget', () => {
    storage.setItem(
      'product-factory:idea-draft',
      JSON.stringify({
        version: 1,
        draft: {
          name: 42,
          idea: 'Valid idea',
          targetUser: null,
          problem: ['invalid'],
          expectedOutput: 'Prototype',
          workspacePath: '/tmp/project',
          budgetUsd: 0,
        },
      })
    );

    expect(loadProductIdeaDraft(storage)).toEqual({
      name: '',
      idea: 'Valid idea',
      targetUser: '',
      problem: '',
      expectedOutput: 'Prototype',
      workspacePath: '/tmp/project',
      budgetUsd: undefined,
    });
  });

  it('saves a trimmed draft with the current storage version', () => {
    saveProductIdeaDraft(
      {
        name: '  Research Copilot  ',
        idea: '  Build a research copilot  ',
        targetUser: '  Product managers  ',
        problem: '',
        expectedOutput: '  Desktop prototype  ',
        workspacePath: '  /tmp/research  ',
        budgetUsd: 8,
      },
      storage
    );

    expect(JSON.parse(storage.getItem('product-factory:idea-draft') ?? '')).toEqual({
      version: 1,
      draft: {
        name: 'Research Copilot',
        idea: 'Build a research copilot',
        targetUser: 'Product managers',
        problem: '',
        expectedOutput: 'Desktop prototype',
        workspacePath: '/tmp/research',
        budgetUsd: 8,
      },
    });
  });

  it('clears the persisted draft', () => {
    saveProductIdeaDraft({ ...EMPTY_PRODUCT_IDEA_DRAFT, name: 'Temporary', idea: 'Temporary idea' }, storage);

    clearProductIdeaDraft(storage);

    expect(storage.getItem('product-factory:idea-draft')).toBeNull();
  });
});
