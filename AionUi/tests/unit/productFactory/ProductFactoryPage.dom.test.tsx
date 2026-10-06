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
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => translations[key] ?? key,
  }),
}));

import ProductFactoryPage from '@/renderer/pages/product-factory';

const renderPage = (path = '/product-factory') =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <ProductFactoryPage />
    </MemoryRouter>
  );

describe('ProductFactoryPage', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    cleanup();
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
    expect(screen.getByText('Draft')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue draft' })).toBeInTheDocument();
    expect(screen.queryByText('No product runs yet')).not.toBeInTheDocument();
  });
});
