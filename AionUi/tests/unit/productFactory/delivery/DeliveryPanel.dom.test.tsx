import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import DeliveryPanel from '@/renderer/pages/product-factory/components/Delivery';
import type { ProductFactoryDelivery, ProductFactoryRun } from '@/renderer/pages/product-factory/types';
import { getProductFactoryCostReport, repriceProductFactoryCosts } from '@/renderer/pages/product-factory/client';
import { downloadTextContent } from '@renderer/utils/file/download';

vi.mock('@/renderer/pages/product-factory/client', () => ({
  getProductFactoryCostReport: vi.fn(),
  repriceProductFactoryCosts: vi.fn(),
}));
vi.mock('@renderer/utils/file/download', () => ({ downloadTextContent: vi.fn() }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const run: ProductFactoryRun = {
  id: 'run-1',
  name: 'Title tool',
  idea: 'Generate titles',
  targetUser: 'Makers',
  problem: 'Manual work',
  expectedOutput: 'A prototype',
  workspacePath: '/workspace/title-tool',
  status: 'completed',
  teamId: 'team-1',
  createdAt: 0,
  updatedAt: 1,
};

const delivery = {
  run,
  ready: true,
  workspacePath: '/workspace/title-tool',
  taskCounts: { total: 1, pending: 0, inProgress: 0, inReview: 0, completed: 1, failed: 0 },
  usageSummary: {
    teamId: 'team-1',
    inputTokens: 1200,
    outputTokens: 300,
    costEst: 0.24,
    costUnknown: false,
    budgetLimitUsd: 1,
    budgetRemainingUsd: 0.76,
    budgetExceeded: false,
    tasks: [
      {
        taskId: 'team-1-test-1',
        inputTokens: 1200,
        outputTokens: 300,
        costEst: 0.24,
        turnCount: 2,
      },
    ],
  },
  checks: [{ code: 'run_completed', passed: true, detail: 'completed' }],
} satisfies ProductFactoryDelivery;

describe('Product Factory delivery panel', () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it('shows the root workspace and run-level usage after completion', () => {
    render(<DeliveryPanel delivery={delivery} />);

    expect(screen.getByText('/workspace/title-tool')).toBeInTheDocument();
    expect(screen.getByText('team-1-test-1')).toBeInTheDocument();
    expect(screen.getByText('1200')).toBeInTheDocument();
    expect(screen.getByText('$0.2400')).toBeInTheDocument();
  });

  it('shows failed delivery checks without claiming readiness', () => {
    render(
      <DeliveryPanel
        delivery={{
          ...delivery,
          ready: false,
          checks: [{ code: 'start_guide_exists', passed: false, detail: 'missing' }],
        }}
      />
    );
    expect(screen.getByText('productFactory.delivery.notReady')).toBeInTheDocument();
    expect(screen.getByText('productFactory.delivery.checks.start_guide_exists')).toBeInTheDocument();
    expect(screen.queryByText('productFactory.delivery.ready')).not.toBeInTheDocument();
  });

  it('shows a proven zero admission cost while explaining counts are not model calls', () => {
    render(
      <DeliveryPanel
        delivery={{
          ...delivery,
          usageSummary: { ...delivery.usageSummary, costEst: 0, costUnknown: false, tasks: [] },
        }}
      />
    );
    expect(screen.getByText('$0.0000')).toBeInTheDocument();
    expect(screen.getByText('productFactory.delivery.attemptCountHint')).toBeInTheDocument();
    expect(screen.queryByText('productFactory.delivery.unknownCost')).not.toBeInTheDocument();
  });

  it('exports task costs while preserving unknown amounts', async () => {
    vi.mocked(getProductFactoryCostReport).mockResolvedValue({
      run,
      taskCounts: delivery.taskCounts,
      usageSummary: { ...delivery.usageSummary, costUnknown: true, costEst: undefined },
      generatedAt: 42,
      usage: [
        {
          id: 'usage-1',
          taskId: 'team-1-test-1',
          inputTokens: 1200,
          outputTokens: 300,
          cachedReadTokens: 1000,
          cachedWriteTokens: 0,
          costUnknownReason: 'model_price_missing',
          conversationId: 'c',
          turnId: 'turn',
          createdAt: 40,
        },
      ],
    });
    render(<DeliveryPanel delivery={delivery} />);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.exportCostReport' }));
    await waitFor(() => expect(downloadTextContent).toHaveBeenCalledOnce());
    const payload = JSON.parse(vi.mocked(downloadTextContent).mock.calls[0][0]);
    expect(payload.version).toBe(2);
    expect(payload.turns[0].cachedReadTokens).toBe(1000);
    expect(payload.turns[0].costEst).toBeNull();
    expect(payload.turns[0].costUnknownReason).toBe('model_price_missing');
    expect(payload.usage.costEst).toBeNull();
    expect(payload.usage.tasks[0].costEst).toBe(0.24);
  });

  it('reports a failed export without downloading an empty report', async () => {
    vi.mocked(getProductFactoryCostReport).mockRejectedValue(new Error('unavailable'));
    render(<DeliveryPanel delivery={delivery} />);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.exportCostReport' }));
    await waitFor(() => expect(screen.getByText('productFactory.delivery.exportError')).toBeInTheDocument());
    expect(downloadTextContent).not.toHaveBeenCalled();
  });

  it('exports planning usage separately with its Send attempt and unknown cost intact', async () => {
    vi.mocked(getProductFactoryCostReport).mockResolvedValue({
      run,
      taskCounts: delivery.taskCounts,
      usageSummary: delivery.usageSummary,
      generatedAt: 42,
      planningUsage: [
        {
          planningAttemptId: 'plan-1',
          phase: 'blueprint',
          state: 'failed',
          costUnknown: true,
          usage: [
            {
              id: 'u',
              inputTokens: 2,
              outputTokens: 1,
              conversationId: 'c',
              turnId: 'app-turn',
              attemptId: 'send-2',
              costUnknownReason: 'model_not_priced',
              createdAt: 1,
            },
          ],
        },
      ],
    });
    render(<DeliveryPanel delivery={delivery} />);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.exportCostReport' }));
    await waitFor(() => expect(downloadTextContent).toHaveBeenCalledOnce());
    const payload = JSON.parse(vi.mocked(downloadTextContent).mock.calls[0][0]);
    expect(payload.planningUsage[0].planningAttemptId).toBe('plan-1');
    expect(payload.planningUsage[0].usage[0]).toMatchObject({
      attemptId: 'send-2',
      costEst: null,
      costUnknownReason: 'model_not_priced',
    });
  });
  it('refreshes amount and delivery readiness after explicit re-estimation', async () => {
    vi.mocked(repriceProductFactoryCosts).mockResolvedValue({ updatedCount: 1, skippedCount: 0, delivery });
    render(
      <DeliveryPanel
        delivery={{
          ...delivery,
          ready: false,
          usageSummary: { ...delivery.usageSummary, costEst: undefined, costUnknown: true },
        }}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.reprice' }));
    await waitFor(() => expect(screen.getByText('productFactory.delivery.ready')).toBeInTheDocument());
    expect(repriceProductFactoryCosts).toHaveBeenCalledWith('run-1');
    expect(screen.getByText('$0.2400')).toBeInTheDocument();
  });

  it('preserves unknown amounts when complete prices are unavailable', async () => {
    const unknownDelivery = {
      ...delivery,
      ready: false,
      usageSummary: { ...delivery.usageSummary, costEst: undefined, costUnknown: true },
    };
    vi.mocked(repriceProductFactoryCosts).mockResolvedValue({
      updatedCount: 0,
      skippedCount: 1,
      delivery: unknownDelivery,
    });
    render(<DeliveryPanel delivery={unknownDelivery} />);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.reprice' }));
    await waitFor(() => expect(screen.getByText('productFactory.delivery.repriceResult')).toBeInTheDocument());
    expect(screen.getByText('productFactory.delivery.notReady')).toBeInTheDocument();
    expect(screen.queryByText('$0.0000')).not.toBeInTheDocument();
  });

  it('shows a re-estimation failure while preserving the last delivery', async () => {
    vi.mocked(repriceProductFactoryCosts).mockRejectedValue(new Error('unavailable'));
    render(<DeliveryPanel delivery={delivery} />);
    fireEvent.click(screen.getByRole('button', { name: 'productFactory.delivery.reprice' }));
    await waitFor(() => expect(screen.getByText('productFactory.delivery.repriceError')).toBeInTheDocument());
    expect(screen.getByText('$0.2400')).toBeInTheDocument();
  });
});
