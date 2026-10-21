/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

const translations: Record<string, string> = {
  'productFactory.home.eyebrow': 'AI product development',
  'productFactory.home.title': 'Turn an idea into an executable product plan',
  'productFactory.home.description': 'Clarify the requirement and hand execution to your Agent team.',
  'productFactory.home.createProduct': 'Create product',
  'productFactory.home.continueDraft': 'Continue draft',
  'productFactory.home.emptyTitle': 'No product runs yet',
  'productFactory.home.emptyDescription': 'Start with one sentence.',
  'productFactory.pipeline.title': 'Product pipeline',
  'productFactory.pipeline.steps.idea': 'Idea',
  'productFactory.pipeline.steps.interview': 'Clarify',
  'productFactory.pipeline.steps.blueprint': 'Blueprint',
  'productFactory.pipeline.steps.tasks': 'Task plan',
  'productFactory.pipeline.steps.execution': 'Build',
  'productFactory.pipeline.steps.review': 'Review',
  'productFactory.pipeline.steps.delivery': 'Deliver',
  'productFactory.form.title': 'Describe the product you want to build',
  'productFactory.form.description': 'Only the product name and idea are required now.',
  'productFactory.form.nameLabel': 'Product name',
  'productFactory.form.namePlaceholder': 'For example: Research Copilot',
  'productFactory.form.ideaLabel': 'Product idea',
  'productFactory.form.ideaPlaceholder': 'Describe the product idea',
  'productFactory.form.targetUserLabel': 'Target user',
  'productFactory.form.targetUserPlaceholder': 'For example: product managers',
  'productFactory.form.problemLabel': 'Problem to solve',
  'productFactory.form.problemPlaceholder': 'What is unreliable today?',
  'productFactory.form.expectedOutputLabel': 'Expected output',
  'productFactory.form.expectedOutputPlaceholder': 'For example: a runnable prototype',
  'productFactory.form.workspaceLabel': 'Workspace path',
  'productFactory.form.workspacePlaceholder': 'Choose a directory later',
  'productFactory.form.budgetLabel': 'Budget (USD)',
  'productFactory.form.budgetPlaceholder': 'Optional',
  'productFactory.form.budgetHint': 'The backend budget guard pauses new Agent turns.',
  'productFactory.form.saveDraft': 'Save draft',
  'productFactory.form.startAnalysis': 'Start requirement analysis',
  'productFactory.form.back': 'Back to projects',
  'productFactory.form.saved': 'Draft saved',
  'productFactory.form.validation.nameRequired': 'Enter a product name',
  'productFactory.form.validation.ideaRequired': 'Describe the product idea',
  'productFactory.status.draft': 'Draft',
  'productFactory.handoff.statuses.taskDraftReady': 'Task plan ready',
  'common.status': 'Status',
  'productFactory.home.localDraft': 'Local unsubmitted draft',
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => translations[key] ?? key,
  }),
}));

import ProductFactoryPage from '@/renderer/pages/product-factory';
import FactoryHome from '@/renderer/pages/product-factory/components/FactoryHome';
import type { ProductFactoryRun } from '@/renderer/pages/product-factory/types';
import * as client from '@/renderer/pages/product-factory/client';

const renderPage = (path = '/product-factory') =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <ProductFactoryPage />
    </MemoryRouter>
  );

describe('ProductFactoryPage', () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.spyOn(client, 'listProductFactoryRuns').mockResolvedValue([]);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('keeps invalid budget input and blocks saving it as an unset budget', async () => {
    const create = vi.spyOn(client, 'createProductFactoryRun');
    renderPage('/product-factory/new');
    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'Budget example' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: 'Build a tool' } });
    fireEvent.change(screen.getByTestId('product-factory-budget'), { target: { value: '-5' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    expect(screen.getByTestId('product-factory-budget')).toHaveValue('-5');
    expect(await screen.findByText('productFactory.form.validation.budgetInvalid')).toBeInTheDocument();
    expect(create).not.toHaveBeenCalled();
  });

  it('retains an unfinished idea across unmount without requiring a manual save', async () => {
    const first = renderPage('/product-factory/new');
    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'Unsubmitted idea' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: 'Keep my input' } });
    first.unmount();
    renderPage('/product-factory/new');
    expect(screen.getByTestId('product-factory-name')).toHaveValue('Unsubmitted idea');
    expect(screen.getByTestId('product-factory-idea')).toHaveValue('Keep my input');
  });
  it('offers recovery even when only an optional field was entered', async () => {
    const first = renderPage('/product-factory/new');
    fireEvent.change(screen.getByPlaceholderText('For example: product managers'), {
      target: { value: 'Researchers only' },
    });
    first.unmount();
    renderPage();
    expect(screen.getByTestId('product-idea-draft-card')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Continue draft' }));
    expect(await screen.findByPlaceholderText('For example: product managers')).toHaveValue('Researchers only');
  });

  it('locks the idea inputs while a save is pending and retains them after failure', async () => {
    let rejectSave!: (reason: Error) => void;
    vi.spyOn(client, 'createProductFactoryRun').mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          rejectSave = reject;
        })
    );
    renderPage('/product-factory/new');
    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'Pending save' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: 'Keep the current input' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    expect(screen.getByTestId('product-factory-name')).toBeDisabled();
    expect(screen.getByTestId('product-factory-budget')).toBeDisabled();
    rejectSave(new Error('offline'));
    await waitFor(() => expect(screen.getByTestId('product-factory-name')).toBeEnabled());
    expect(screen.getByTestId('product-factory-name')).toHaveValue('Pending save');
    expect(JSON.parse(localStorage.getItem('product-factory:idea-draft') ?? '').draft.name).toBe('Pending save');
  });

  it('lists older products without replacing a separate local idea draft', async () => {
    const draft = {
      name: 'Local idea',
      idea: 'Unsubmitted text',
      targetUser: '',
      problem: '',
      expectedOutput: '',
      workspacePath: '',
    };
    localStorage.setItem('product-factory:idea-draft', JSON.stringify({ version: 1, draft }));
    vi.mocked(client.listProductFactoryRuns).mockResolvedValue([
      {
        ...draft,
        id: 'newer',
        name: 'Recent product',
        status: 'running',
        createdAt: 2,
        updatedAt: 2,
      } as ProductFactoryRun,
      {
        ...draft,
        id: 'older',
        name: 'Older product',
        status: 'completed',
        createdAt: 1,
        updatedAt: 1,
      } as ProductFactoryRun,
    ]);
    renderPage();
    expect(await screen.findByText('Older product')).toBeInTheDocument();
    expect(screen.getByText('Local idea')).toBeInTheDocument();
    expect(JSON.parse(localStorage.getItem('product-factory:idea-draft') ?? '').draft.name).toBe('Local idea');
  });

  it('describes the product state with localized copy instead of backend identifiers', () => {
    const draft = {
      name: 'Research Copilot',
      idea: 'Create a research brief',
      targetUser: '',
      problem: '',
      expectedOutput: '',
      workspacePath: '',
    };
    render(
      <FactoryHome
        draft={draft}
        runs={[{ ...draft, status: 'task_draft_ready' } as ProductFactoryRun]}
        loadingRuns={false}
        apiUnavailable={false}
        onRetry={vi.fn()}
        onCreate={vi.fn()}
        onContinue={vi.fn()}
      />
    );
    expect(screen.getByText('Task plan ready')).toBeInTheDocument();
    expect(screen.queryByText('task_draft_ready')).toBeNull();
  });

  it('shows the pipeline and an actionable empty state for a first-time user', () => {
    renderPage();

    expect(screen.getByText('Turn an idea into an executable product plan')).toBeInTheDocument();
    expect(screen.getByText('No product runs yet')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Create product' })).toHaveLength(2);
    expect(screen.getByText('Blueprint')).toBeInTheDocument();
    expect(screen.getByText('Deliver')).toBeInTheDocument();
  });

  it('shows required-field errors and does not persist an empty draft', () => {
    renderPage('/product-factory/new');

    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));

    expect(screen.getByText('Enter a product name')).toBeInTheDocument();
    expect(screen.getByText('Describe the product idea')).toBeInTheDocument();
    expect(window.localStorage.getItem('product-factory:idea-draft')).toBeNull();
  });

  it('saves a valid product idea draft', async () => {
    renderPage('/product-factory/new');

    fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: 'Research Copilot' } });
    fireEvent.change(screen.getByTestId('product-factory-idea'), {
      target: { value: 'Turn interview notes into a research brief' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));

    const stored = JSON.parse(window.localStorage.getItem('product-factory:idea-draft') ?? '');
    expect(stored.draft.name).toBe('Research Copilot');
    expect(stored.draft.idea).toBe('Turn interview notes into a research brief');
    await waitFor(() => expect(screen.getByText('Draft saved')).toBeInTheDocument());
  });

  it('does not hide a committed run when remembering its navigation id fails', async () => {
    const created: ProductFactoryRun = {
      id: 'created-run',
      name: 'Committed product',
      idea: 'Keep the committed run',
      targetUser: '',
      problem: '',
      expectedOutput: '',
      workspacePath: '/workspace/committed',
      status: 'draft',
      createdAt: 1,
      updatedAt: 1,
    };
    vi.spyOn(client, 'createProductFactoryRun').mockResolvedValue(created);
    vi.spyOn(client, 'getProductFactoryRun').mockResolvedValue(created);
    const original = Storage.prototype.setItem;
    const storage = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(function (
      this: Storage,
      key: string,
      value: string
    ) {
      if (key === 'product-factory:run-id') throw new DOMException('quota', 'QuotaExceededError');
      original.call(this, key, value);
    });
    try {
      renderPage('/product-factory/new');
      fireEvent.change(screen.getByTestId('product-factory-name'), { target: { value: created.name } });
      fireEvent.change(screen.getByTestId('product-factory-idea'), { target: { value: created.idea } });
      fireEvent.click(screen.getByRole('button', { name: 'Start requirement analysis' }));
      await screen.findByRole('heading', { name: created.name });
      expect(client.createProductFactoryRun).toHaveBeenCalledTimes(1);
      expect(screen.queryByText('productFactory.form.startError')).not.toBeInTheDocument();
    } finally {
      storage.mockRestore();
    }
  });

  it('restores the saved draft when the form is reopened', () => {
    window.localStorage.setItem(
      'product-factory:idea-draft',
      JSON.stringify({
        version: 1,
        draft: {
          name: 'Research Copilot',
          idea: 'Turn interview notes into a research brief',
          targetUser: 'Product managers',
          problem: '',
          expectedOutput: '',
          workspacePath: '/tmp/research',
          budgetUsd: 10,
        },
      })
    );

    renderPage('/product-factory/new');

    expect(screen.getByTestId('product-factory-name')).toHaveValue('Research Copilot');
    expect(screen.getByTestId('product-factory-idea')).toHaveValue('Turn interview notes into a research brief');
    expect(screen.getByTestId('product-factory-workspace')).toHaveValue('/tmp/research');
    expect(screen.getByTestId('product-factory-budget')).toHaveValue('10');
  });

  it('shows a saved draft card on the home page', () => {
    window.localStorage.setItem(
      'product-factory:idea-draft',
      JSON.stringify({
        version: 1,
        draft: {
          name: 'Research Copilot',
          idea: 'Turn interview notes into a research brief',
          targetUser: '',
          problem: '',
          expectedOutput: '',
          workspacePath: '',
        },
      })
    );

    renderPage();

    expect(screen.getByText('Research Copilot')).toBeInTheDocument();
    expect(screen.getByText('Turn interview notes into a research brief')).toBeInTheDocument();
    expect(screen.getByText('Local unsubmitted draft')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue draft' })).toBeInTheDocument();
    expect(screen.queryByText('No product runs yet')).not.toBeInTheDocument();
  });
});
