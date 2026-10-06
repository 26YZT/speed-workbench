/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';
import type { ProductFactoryRun, TaskDraftArtifact } from '@/renderer/pages/product-factory/types';

const {
  getProductFactoryRun,
  saveProductFactoryBlueprint,
  confirmProductFactoryBlueprint,
  generateProductFactoryTaskDraft,
  saveProductFactoryTaskDraft,
  confirmProductFactoryTaskDraft,
  listProductFactoryPlanning,
  getProductFactoryCostReport,
} = vi.hoisted(() => ({
  getProductFactoryRun: vi.fn(),
  saveProductFactoryBlueprint: vi.fn(),
  confirmProductFactoryBlueprint: vi.fn(),
  generateProductFactoryTaskDraft: vi.fn(),
  saveProductFactoryTaskDraft: vi.fn(),
  confirmProductFactoryTaskDraft: vi.fn(),
  listProductFactoryPlanning: vi.fn().mockResolvedValue([]),
  getProductFactoryCostReport: vi.fn().mockRejectedValue(new Error('cost not available')),
}));

vi.mock('@/renderer/pages/product-factory/client', () => ({
  listProductFactoryPlanning,
  getProductFactoryCostReport,
  getProductFactoryRun,
  listProductFactoryRuns: vi.fn().mockResolvedValue([]),
  createProductFactoryRun: vi.fn(),
  saveProductFactoryInterview: vi.fn(),
  confirmProductFactoryInterview: vi.fn(),
  saveProductFactoryBlueprint,
  confirmProductFactoryBlueprint,
  generateProductFactoryTaskDraft,
  saveProductFactoryTaskDraft,
  confirmProductFactoryTaskDraft,
}));

vi.mock('@/renderer/pages/product-factory/components/Delivery/VersionPanel', () => ({ default: () => null }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'productFactory.workspace.notFound': 'Run not found',
        'productFactory.workspace.retry': 'Retry',
        'productFactory.workspace.loading': 'Loading run',
        'productFactory.workspace.actionError': 'Update failed',
        'productFactory.form.back': 'Back',
        'productFactory.home.eyebrow': 'AI product development',
        'productFactory.interview.title': 'Interview',
        'productFactory.interview.description': 'Clarify the idea',
        'productFactory.interview.notSure': 'Not sure',
        'productFactory.interview.summaryLabel': 'Summary',
        'productFactory.interview.save': 'Save interview',
        'productFactory.interview.confirm': 'Confirm interview',
        'productFactory.interview.questions.audience': 'Who uses {{name}}?',
        'productFactory.interview.questions.problem': 'What problem?',
        'productFactory.interview.questions.output': 'What output?',
        'productFactory.blueprint.title': 'Blueprint',
        'productFactory.blueprint.description': 'Review the product structure',
        'productFactory.blueprint.risks': 'Risks',
        'productFactory.blueprint.openQuestions': 'Open questions',
        'productFactory.blueprint.save': 'Save blueprint',
        'productFactory.blueprint.confirm': 'Confirm blueprint',
      })[key] ?? key,
  }),
}));

import ProductFactoryPage from '@/renderer/pages/product-factory';

const taskDraft: TaskDraftArtifact = {
  version: 1,
  generatedBy: 'draft',
  confirmed: false,
  revision: 3,
  updatedAt: 0,
  tasks: [
    {
      id: 'data-1',
      title: 'Define data',
      description: 'Persist the contract',
      type: 'data',
      blockedBy: [],
      acceptanceCriteria: ['Can be loaded'],
      suggestedRole: 'backend',
      effort: 'medium',
    },
  ],
};

const taskRun: ProductFactoryRun = {
  id: 'run-1',
  name: 'Research Copilot',
  idea: 'Turn notes into a plan',
  targetUser: 'Product managers',
  problem: 'Notes are scattered',
  expectedOutput: 'A blueprint',
  workspacePath: '',
  status: 'task_draft_ready',
  taskDraft,
  createdAt: 0,
  updatedAt: 0,
};

const renderWorkspace = () =>
  render(
    <MemoryRouter initialEntries={['/product-factory/run-1']}>
      <ProductFactoryPage />
    </MemoryRouter>
  );

describe('ProductFactoryPage workspace loading', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('surfaces the run loading error and keeps retry available', async () => {
    getProductFactoryRun.mockRejectedValueOnce(new Error('backend unavailable'));

    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );

    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('backend unavailable'));
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
  });

  it('renders a blueprint returned with backend field names', async () => {
    getProductFactoryRun.mockResolvedValueOnce({
      id: 'run-1',
      name: 'Research Copilot',
      idea: 'Turn notes into a plan',
      targetUser: 'Product managers',
      problem: 'Notes are scattered',
      expectedOutput: 'A blueprint',
      workspacePath: '',
      status: 'blueprint_ready',
      blueprint: {
        version: 1,
        sections: [{ id: 'goals', title: 'Goals', content: 'Clarify the goal', confirmed: false }],
        risks: [],
        open_questions: [],
        generated_by: 'draft',
        confirmed: false,
        updated_at: Date.now(),
      },
      createdAt: Date.now(),
      updatedAt: Date.now(),
    });

    render(
      <MemoryRouter initialEntries={['/product-factory/run-1']}>
        <ProductFactoryPage />
      </MemoryRouter>
    );

    await waitFor(() => expect(screen.getByRole('heading', { name: 'Blueprint' })).toBeInTheDocument());
    expect(screen.getAllByText('Goals')).toHaveLength(2);
    expect(screen.getByText('Risks')).toBeInTheDocument();
  });

  it('renders applied model questions using their wire names and keeps human confirmation required', async () => {
    getProductFactoryRun.mockResolvedValueOnce({
      ...taskRun,
      status: 'interviewing',
      taskDraft: undefined,
      interview: {
        version: 2,
        generated_by: 'model',
        confirmed: false,
        summary: '',
        questions: [{ id: 'outcome', question: 'Which result matters?', reason: 'Define success', answer: '' }],
      },
    });
    renderWorkspace();
    expect(await screen.findByText('Which result matters?')).toBeInTheDocument();
    expect(screen.getByText('Define success')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Confirm interview' })).toBeDisabled();
  });
});

describe('ProductFactoryPage task draft workflow', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    listProductFactoryPlanning.mockResolvedValue([]);
    getProductFactoryCostReport.mockRejectedValue(new Error('cost not available'));
    window.localStorage.clear();
    getProductFactoryRun.mockResolvedValue(taskRun);
    saveProductFactoryTaskDraft.mockImplementation(async (_id: string, draft: TaskDraftArtifact) => ({
      ...taskRun,
      taskDraft: { ...draft, revision: draft.revision + 1 },
    }));
  });

  afterEach(cleanup);

  it('offers generation only after blueprint confirmation and opens the generated tasks', async () => {
    const blueprintRun: ProductFactoryRun = {
      ...taskRun,
      status: 'blueprint_ready',
      taskDraft: undefined,
      blueprint: {
        version: 1,
        sections: [{ id: 'goals', title: 'Goals', content: 'Clarify the goal', confirmed: false }],
        risks: [],
        openQuestions: [],
        generatedBy: 'draft',
        confirmed: false,
        updatedAt: 0,
      },
    };
    const confirmedRun = { ...blueprintRun, blueprint: { ...(blueprintRun.blueprint as object), confirmed: true } };
    getProductFactoryRun.mockResolvedValue(blueprintRun);
    saveProductFactoryBlueprint.mockResolvedValue(confirmedRun);
    confirmProductFactoryBlueprint.mockResolvedValue(confirmedRun);
    generateProductFactoryTaskDraft.mockResolvedValue(taskRun);
    renderWorkspace();
    await screen.findByRole('button', { name: 'Confirm blueprint' });
    expect(screen.queryByRole('button', { name: 'productFactory.taskDraft.generate' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Confirm blueprint' }));
    fireEvent.click(await screen.findByRole('button', { name: 'productFactory.taskDraft.generate' }));
    expect(await screen.findByPlaceholderText('productFactory.taskDraft.titleField')).toHaveValue('Define data');
    expect(generateProductFactoryTaskDraft).toHaveBeenCalledWith('run-1');
  });

  it('writes consecutive edits back to the rendered run and saves the latest values', async () => {
    renderWorkspace();
    const title = await screen.findByPlaceholderText('productFactory.taskDraft.titleField');
    fireEvent.change(title, { target: { value: 'Define storage' } });
    fireEvent.change(screen.getByPlaceholderText('productFactory.taskDraft.descriptionField'), {
      target: { value: 'Persist edited data' },
    });
    expect(title).toHaveValue('Define storage');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.save' }));
    await waitFor(() =>
      expect(saveProductFactoryTaskDraft).toHaveBeenCalledWith(
        'run-1',
        expect.objectContaining({
          revision: 3,
          tasks: [expect.objectContaining({ title: 'Define storage', description: 'Persist edited data' })],
        })
      )
    );
    expect(title).toHaveValue('Define storage');
  });

  it('waits for the latest edits to be saved before confirming with the returned revision', async () => {
    let resolveSave!: (run: ProductFactoryRun) => void;
    saveProductFactoryTaskDraft.mockReturnValue(
      new Promise<ProductFactoryRun>((resolve) => {
        resolveSave = resolve;
      })
    );
    const edited = { ...taskDraft, tasks: [{ ...taskDraft.tasks[0], title: 'Latest title' }] };
    confirmProductFactoryTaskDraft.mockResolvedValue({
      ...taskRun,
      taskDraft: { ...edited, revision: 4, confirmed: true },
    });
    renderWorkspace();
    fireEvent.change(await screen.findByPlaceholderText('productFactory.taskDraft.titleField'), {
      target: { value: 'Latest title' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.confirm' }));
    expect(saveProductFactoryTaskDraft).toHaveBeenCalledWith('run-1', edited);
    expect(confirmProductFactoryTaskDraft).not.toHaveBeenCalled();
    await act(async () => {
      resolveSave({ ...taskRun, taskDraft: { ...edited, revision: 4 } });
    });
    expect(confirmProductFactoryTaskDraft).toHaveBeenCalledWith('run-1', 4);
    expect(screen.getByPlaceholderText('productFactory.taskDraft.titleField')).toBeDisabled();
  });

  it('does not confirm when saving fails and retains local edits for retry', async () => {
    saveProductFactoryTaskDraft.mockRejectedValueOnce(new Error('Save failed'));
    renderWorkspace();
    const title = await screen.findByPlaceholderText('productFactory.taskDraft.titleField');
    fireEvent.change(title, { target: { value: 'Keep my edits' } });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.confirm' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Save failed');
    expect(confirmProductFactoryTaskDraft).not.toHaveBeenCalled();
    expect(title).toHaveValue('Keep my edits');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.save' }));
    await waitFor(() => expect(saveProductFactoryTaskDraft).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(saveProductFactoryTaskDraft).toHaveBeenLastCalledWith(
        'run-1',
        expect.objectContaining({
          revision: 3,
          tasks: [expect.objectContaining({ title: 'Keep my edits' })],
        })
      )
    );
  });

  it('retains the saved revision when confirmation fails so retry can save again', async () => {
    confirmProductFactoryTaskDraft.mockRejectedValueOnce(new Error('Confirm failed'));
    renderWorkspace();
    fireEvent.change(await screen.findByPlaceholderText('productFactory.taskDraft.titleField'), {
      target: { value: 'Saved title' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.confirm' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Confirm failed');
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.taskDraft.save' }));
    await waitFor(() =>
      expect(saveProductFactoryTaskDraft).toHaveBeenLastCalledWith(
        'run-1',
        expect.objectContaining({
          revision: 4,
          tasks: [expect.objectContaining({ title: 'Saved title' })],
        })
      )
    );
  });
});
