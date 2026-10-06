import { describe, expect, it } from 'vitest';
import { buildCostReportExport } from '@/renderer/pages/product-factory/components/Delivery/costReport';
import type { FactoryTaskUsage, ProductFactoryCostReport } from '@/renderer/pages/product-factory/types';
import { run } from '../planning/fixtures';

const notSent: FactoryTaskUsage = {
  id: 'not-sent',
  inputTokens: 0,
  outputTokens: 0,
  costEst: 0,
  costSource: 'not_sent',
  cachedReadTokens: 0,
  cachedWriteTokens: 0,
  conversationId: 'c',
  turnId: 't',
  attemptId: 'send-1',
  createdAt: 1,
};
const report: ProductFactoryCostReport = {
  run,
  generatedAt: 1,
  usage: [notSent],
  taskCounts: { total: 0, pending: 0, inProgress: 0, inReview: 0, completed: 0, failed: 0 },
  usageSummary: {
    teamId: '',
    inputTokens: 0,
    outputTokens: 0,
    costEst: 0,
    costUnknown: false,
    budgetExceeded: false,
    tasks: [{ taskId: 'task', inputTokens: 0, outputTokens: 0, costEst: 0, turnCount: 1 }],
  },
};

describe('cost ledger export semantics', () => {
  it('preserves a verified not_sent zero and its original source', () => {
    const exported = buildCostReportExport(report);
    expect(exported.turns[0]).toMatchObject({
      costEst: 0,
      costSource: 'not_sent',
      costUnknownReason: null,
      attemptId: 'send-1',
    });
    expect(exported.usage.costEst).toBe(0);
  });

  it('keeps the legacy count with an attemptCount alias and an explicit meaning', () => {
    const exported = buildCostReportExport(report);
    expect(exported.usage.tasks[0]).toMatchObject({ turnCount: 1, attemptCount: 1 });
    expect(exported.attemptCountMeaning).toBe('ledger_records_including_verified_not_sent_not_model_call_count');
  });

  it('exports unknown billing records as null alongside known not_sent zero', () => {
    const exported = buildCostReportExport({
      ...report,
      usage: [
        notSent,
        {
          ...notSent,
          id: 'unknown',
          costEst: undefined,
          costSource: undefined,
          costUnknownReason: 'billing_attribution_uncertain',
        },
      ],
      usageSummary: { ...report.usageSummary, costEst: undefined, costUnknown: true },
    });
    expect(exported.turns[0].costEst).toBe(0);
    expect(exported.turns[1]).toMatchObject({ costEst: null, costUnknownReason: 'billing_attribution_uncertain' });
    expect(exported.usage.costEst).toBeNull();
  });

  it('includes unattributed member costs and deduplicates grouped planning projections', () => {
    const exported = buildCostReportExport({
      ...report,
      usageSummary: { ...report.usageSummary, unassignedUsage: [notSent, { ...notSent, id: 'member' }] },
      planningUsage: [
        { planningAttemptId: 'p', phase: 'blueprint', state: 'failed', costUnknown: false, usage: [notSent] },
      ],
    });
    expect(exported.attemptCount).toBe(2);
    expect(exported.turns.map((attempt) => attempt.id)).toEqual(['not-sent', 'member']);
  });
});
