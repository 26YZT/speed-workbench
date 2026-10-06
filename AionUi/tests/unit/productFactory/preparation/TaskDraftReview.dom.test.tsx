/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import TaskDraftReview from '@/renderer/pages/product-factory/components/Preparation/TaskDraftReview';
import type { TaskDraftArtifact } from '@/renderer/pages/product-factory/types';

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

const artifact: TaskDraftArtifact = {
  version: 1,
  generatedBy: 'draft',
  confirmed: false,
  revision: 1,
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

describe('TaskDraftReview', () => {
  it('locks edits while saving so an in-flight response cannot discard newer input', () => {
    const onChange = vi.fn();
    render(
      <TaskDraftReview
        artifact={artifact}
        onChange={onChange}
        onGenerate={vi.fn()}
        onSave={vi.fn()}
        onConfirm={vi.fn()}
        saving
      />
    );
    expect(screen.getByPlaceholderText('productFactory.taskDraft.titleField')).toBeDisabled();
    expect(screen.getByPlaceholderText('productFactory.taskDraft.descriptionField')).toBeDisabled();
    expect(screen.getByRole('button', { name: 'productFactory.taskDraft.add' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'productFactory.taskDraft.delete' })).toBeDisabled();
  });

  it('edits a task and emits the updated artifact', () => {
    const onChange = vi.fn();
    render(
      <TaskDraftReview
        artifact={artifact}
        onChange={onChange}
        onGenerate={vi.fn()}
        onSave={vi.fn()}
        onConfirm={vi.fn()}
        saving={false}
      />
    );
    fireEvent.change(screen.getByPlaceholderText('productFactory.taskDraft.titleField'), {
      target: { value: 'Define storage' },
    });
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ tasks: [expect.objectContaining({ title: 'Define storage' })] })
    );
  });

  it('keeps confirmed drafts read-only', () => {
    render(
      <TaskDraftReview
        artifact={{ ...artifact, confirmed: true }}
        onChange={vi.fn()}
        onGenerate={vi.fn()}
        onSave={vi.fn()}
        onConfirm={vi.fn()}
        saving={false}
      />
    );
    expect(screen.getByPlaceholderText('productFactory.taskDraft.titleField')).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'productFactory.taskDraft.confirm' })).not.toBeInTheDocument();
    expect(screen.getByText('productFactory.taskDraft.noAgent')).toBeInTheDocument();
  });

  it('shows model v2 scopes and preserves requirement links while editing', () => {
    const onChange = vi.fn();
    render(
      <TaskDraftReview
        artifact={{
          ...artifact,
          version: 2,
          generatedBy: 'model',
          tasks: [{ ...artifact.tasks[0], executionScope: 'task_workspace', requirementIds: ['req-1'] }],
        }}
        onChange={onChange}
        onGenerate={vi.fn()}
        onSave={vi.fn()}
        onConfirm={vi.fn()}
        saving={false}
      />
    );
    expect(screen.getByText('productFactory.planning.modelSource')).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText('productFactory.taskDraft.requirementsPlaceholder'), {
      target: { value: 'req-1, req-2' },
    });
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        version: 2,
        generatedBy: 'model',
        tasks: [expect.objectContaining({ executionScope: 'task_workspace', requirementIds: ['req-1', 'req-2'] })],
      })
    );
  });
});
