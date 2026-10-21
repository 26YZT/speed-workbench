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

export const hasProductIdeaDraft = (draft: ProductIdeaDraft): boolean =>
  Object.values(draft).some((value) =>
    typeof value === 'string'
      ? value.trim().length > 0
      : typeof value === 'number' && Number.isFinite(value) && value > 0
  );

const stringValue = (value: unknown, trim = true): string =>
  typeof value === 'string' ? (trim ? value.trim() : value) : '';

const budgetValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : undefined;

const normalizeDraft = (value: unknown, trim = true): ProductIdeaDraft => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { ...EMPTY_PRODUCT_IDEA_DRAFT };
  }

  const draft = value as Record<string, unknown>;
  return {
    name: stringValue(draft.name, trim),
    idea: stringValue(draft.idea, trim),
    targetUser: stringValue(draft.targetUser, trim),
    problem: stringValue(draft.problem, trim),
    expectedOutput: stringValue(draft.expectedOutput, trim),
    workspacePath: stringValue(draft.workspacePath, trim),
    budgetUsd: budgetValue(draft.budgetUsd),
    ...(typeof draft.budgetInput === 'string' ? { budgetInput: draft.budgetInput } : {}),
  };
};

const defaultStorage = (): ProductIdeaDraftStorage | undefined => {
  try {
    return typeof window === 'undefined' ? undefined : window.localStorage;
  } catch {
    return undefined;
  }
};

export function loadProductIdeaDraft(storage = defaultStorage()): ProductIdeaDraft {
  if (!storage) return { ...EMPTY_PRODUCT_IDEA_DRAFT };

  try {
    const storedValue = storage.getItem(PRODUCT_IDEA_DRAFT_KEY);
    if (!storedValue) return { ...EMPTY_PRODUCT_IDEA_DRAFT };
    const payload = JSON.parse(storedValue) as StoredProductIdeaDraft;
    if (payload.version !== PRODUCT_IDEA_DRAFT_VERSION) {
      return { ...EMPTY_PRODUCT_IDEA_DRAFT };
    }
    return normalizeDraft(payload.draft, false);
  } catch {
    return { ...EMPTY_PRODUCT_IDEA_DRAFT };
  }
}

export function saveProductIdeaDraft(
  draft: ProductIdeaDraft,
  storage = defaultStorage(),
  preserveInput = false
): ProductIdeaDraft {
  if (!storage && typeof window !== 'undefined') throw new Error('LOCAL_DRAFT_STORAGE_UNAVAILABLE');
  const normalized = normalizeDraft(draft, !preserveInput);
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
  storage?.removeItem('product-factory:idea-draft-run-id');
}

export const loadIdeaDraftRunId = (): string => {
  try {
    return localStorage.getItem('product-factory:idea-draft-run-id') ?? '';
  } catch {
    return '';
  }
};
export const saveIdeaDraftRunId = (id: string): void => {
  try {
    localStorage.setItem('product-factory:idea-draft-run-id', id);
  } catch {
    // Navigation metadata must not turn a committed server save into a failure.
  }
};

export function loadProductFactoryRunId(storage = defaultStorage()): string | undefined {
  try {
    const value = storage?.getItem(PRODUCT_FACTORY_RUN_ID_KEY);
    return value?.trim() || undefined;
  } catch {
    return undefined;
  }
}

export function saveProductFactoryRunId(runId: string, storage = defaultStorage()): void {
  try {
    if (runId.trim()) storage?.setItem(PRODUCT_FACTORY_RUN_ID_KEY, runId.trim());
  } catch {
    // The backend run and primary draft remain the sources of truth.
  }
}
