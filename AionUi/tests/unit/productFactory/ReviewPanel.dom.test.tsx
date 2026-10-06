import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import ReviewPanel from '@/renderer/pages/product-factory/components/ReviewPanel';
import type { ProductFactoryReview } from '@/renderer/pages/product-factory/types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const review: ProductFactoryReview = {
  run: {
    id: 'run-1',
    name: 'Title tool',
    idea: 'Generate titles',
    targetUser: 'Makers',
    problem: 'Manual work',
    expectedOutput: 'A prototype',
    workspacePath: '/workspace/title-tool',
    status: 'in_review',
    teamId: 'team-1',
    createdAt: 0,
    updatedAt: 1,
  },
  task: {
    id: 'team-1-data-1',
    teamId: 'team-1',
    subject: 'Define data contract',
    description: 'Persist the title history',
    status: 'in_review',
    owner: 'lead-slot',
    blockedBy: [],
    blocks: ['team-1-backend-1'],
    createdAt: 0,
    updatedAt: 1,
  },
  workspace: { taskId: 'team-1-data-1', path: '/workspace/title-tool/.tasks/data-1' },
  usage: [
    {
      id: 'usage-1',
      taskId: 'team-1-data-1',
      agentId: 'lead-slot',
      model: 'test-model',
      inputTokens: 100,
      outputTokens: 20,
      costEst: 0.01,
      conversationId: 'conv-1',
      turnId: 'turn-1',
      createdAt: 1,
    },
  ],
  usageSummary: {
    teamId: 'team-1',
    inputTokens: 900,
    outputTokens: 150,
    costEst: 0.12,
    costUnknown: false,
    budgetLimitUsd: 1,
    budgetRemainingUsd: 0.88,
    budgetExceeded: false,
    tasks: [],
  },
  acceptanceCriteria: ['The file can be reloaded', 'Invalid data is rejected'],
  nextTask: { id: 'backend-1', title: 'Build API', acceptanceCriteria: ['API responds'] },
  execution: {
    state: 'enqueued',
    teamId: 'team-1',
    taskId: 'team-1-data-1',
    messageId: 'mail-1',
    teamRunId: 'turn-1',
    requestedAt: 1,
    updatedAt: 1,
  },
};

describe('Product Factory review panel', () => {
  afterEach(() => cleanup());

  it('shows evidence and sends human feedback instead of silently dispatching', async () => {
    const onRequestChanges = vi.fn().mockResolvedValue(undefined);
    const onApprove = vi.fn().mockResolvedValue(undefined);
    const onContinue = vi.fn().mockResolvedValue(undefined);
    render(
      <ReviewPanel
        review={review}
        loading={false}
        busy={false}
        onReload={vi.fn()}
        onApprove={onApprove}
        onRequestChanges={onRequestChanges}
        onContinue={onContinue}
      />
    );

    expect(screen.getByText('Define data contract')).toBeInTheDocument();
    expect(screen.getByText('The file can be reloaded')).toBeInTheDocument();
    expect(screen.getByText('/workspace/title-tool/.tasks/data-1')).toBeInTheDocument();
    expect(screen.getByText('900')).toBeInTheDocument();

    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Add a migration check' } });
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.review.requestChanges' }));
    await waitFor(() => expect(onRequestChanges).toHaveBeenCalledWith('Add a migration check'));
    expect(onContinue).not.toHaveBeenCalled();
    expect(onApprove).not.toHaveBeenCalled();
  });

  it('resumes the current task only after an explicit click', async () => {
    const onContinue = vi.fn().mockResolvedValue(undefined);
    render(
      <ReviewPanel
        review={{ ...review, task: { ...review.task, status: 'in_progress' } }}
        loading={false}
        busy={false}
        onReload={vi.fn()}
        onApprove={vi.fn()}
        onRequestChanges={vi.fn()}
        onContinue={onContinue}
      />
    );
    expect(onContinue).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.review.resume' }));
    await waitFor(() => expect(onContinue).toHaveBeenCalledOnce());
  });

  it('does not offer resuming an uncertain dispatch', () => {
    render(
      <ReviewPanel
        review={{
          ...review,
          task: { ...review.task, status: 'in_progress' },
          execution: { ...review.execution!, state: 'uncertain' },
        }}
        loading={false}
        busy={false}
        onReload={vi.fn()}
        onApprove={vi.fn()}
        onRequestChanges={vi.fn()}
        onContinue={vi.fn()}
      />
    );
    expect(screen.queryByRole('button', { name: 'productFactory.review.resume' })).not.toBeInTheDocument();
    expect(screen.getByText('productFactory.execution.uncertain')).toBeInTheDocument();
  });

  it('keeps missing cost unknown instead of showing zero', () => {
    render(
      <ReviewPanel
        review={{ ...review, usageSummary: { ...review.usageSummary, costEst: undefined } }}
        loading={false}
        busy={false}
        onReload={vi.fn()}
        onApprove={vi.fn()}
        onRequestChanges={vi.fn()}
        onContinue={vi.fn()}
      />
    );
    expect(screen.getByText('productFactory.review.unknown')).toBeInTheDocument();
    expect(screen.queryByText('$0.0000')).not.toBeInTheDocument();
  });
});
