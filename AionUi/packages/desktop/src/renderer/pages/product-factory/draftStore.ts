/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import type { ProductIdeaDraft } from './types';

const PRODUCT_IDEA_DRAFT_KEY = 'product-factory:idea-draft';
const PRODUCT_FACTORY_RUN_ID_KEY = 'product-factory:run-id';
const PRODUCT_IDEA_DRAFT_VERSION = 1;

export type ProductIdeaDraftStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

type StoredProductIdeaDraft = {
  version: number;
  draft: unknown;
};

export const EMPTY_PRODUCT_IDEA_DRAFT: ProductIdeaDraft = {
  name: '',
  idea: '',
  targetUser: '',
  problem: '',
  expectedOutput: '',
  workspacePath: '',
  budgetUsd: undefined,
};

const stringValue = (value: unknown): string => (typeof value === 'string' ? value.trim() : '');

const budgetValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : undefined;

const normalizeDraft = (value: unknown): ProductIdeaDraft => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { ...EMPTY_PRODUCT_IDEA_DRAFT };
  }

  const draft = value as Record<string, unknown>;
  return {
    name: stringValue(draft.name),
    idea: stringValue(draft.idea),
    targetUser: stringValue(draft.targetUser),
    problem: stringValue(draft.problem),
    expectedOutput: stringValue(draft.expectedOutput),
    workspacePath: stringValue(draft.workspacePath),
    budgetUsd: budgetValue(draft.budgetUsd),
  };
};

const defaultStorage = (): ProductIdeaDraftStorage | undefined =>
  typeof window === 'undefined' ? undefined : window.localStorage;

export function loadProductIdeaDraft(storage = defaultStorage()): ProductIdeaDraft {
  if (!storage) return { ...EMPTY_PRODUCT_IDEA_DRAFT };

  const storedValue = storage.getItem(PRODUCT_IDEA_DRAFT_KEY);
  if (!storedValue) return { ...EMPTY_PRODUCT_IDEA_DRAFT };

  try {
    const payload = JSON.parse(storedValue) as StoredProductIdeaDraft;
    if (payload.version !== PRODUCT_IDEA_DRAFT_VERSION) {
      return { ...EMPTY_PRODUCT_IDEA_DRAFT };
    }
    return normalizeDraft(payload.draft);
  } catch {
    return { ...EMPTY_PRODUCT_IDEA_DRAFT };
  }
}

export function saveProductIdeaDraft(draft: ProductIdeaDraft, storage = defaultStorage()): ProductIdeaDraft {
  const normalized = normalizeDraft(draft);
  storage?.setItem(
    PRODUCT_IDEA_DRAFT_KEY,
    JSON.stringify({
      version: PRODUCT_IDEA_DRAFT_VERSION,
      draft: normalized,
    })
  );
  return normalized;
}

export function clearProductIdeaDraft(storage = defaultStorage()): void {
  storage?.removeItem(PRODUCT_IDEA_DRAFT_KEY);
}

export function loadProductFactoryRunId(storage = defaultStorage()): string | undefined {
  const value = storage?.getItem(PRODUCT_FACTORY_RUN_ID_KEY);
  return value?.trim() || undefined;
}

export function saveProductFactoryRunId(runId: string, storage = defaultStorage()): void {
  if (runId.trim()) storage?.setItem(PRODUCT_FACTORY_RUN_ID_KEY, runId.trim());
}
