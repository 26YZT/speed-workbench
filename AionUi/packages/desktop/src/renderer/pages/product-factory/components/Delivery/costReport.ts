/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import type { FactoryTaskUsage, ProductFactoryCostReport } from '../../types';

const exportAttempt = (attempt: FactoryTaskUsage) => ({
  ...attempt,
  costEst: attempt.costEst ?? null,
  cachedReadTokens: attempt.cachedReadTokens ?? null,
  cachedWriteTokens: attempt.cachedWriteTokens ?? null,
  costSource: attempt.costSource ?? null,
  pricingSnapshot: attempt.pricingSnapshot ?? null,
  costUnknownReason: attempt.costEst == null ? (attempt.costUnknownReason ?? 'usage_or_pricing_unknown') : null,
});

/** Export ledger evidence without treating admissions as actual model calls. */
export function buildCostReportExport(report: ProductFactoryCostReport) {
  const unique = new Map<string, FactoryTaskUsage>();
  for (const attempt of [
    ...(report.usage ?? []),
    ...(report.usageSummary.unassignedUsage ?? []),
    ...(report.planningUsage ?? []).flatMap((planning) => planning.usage),
  ])
    unique.set(attempt.id, attempt);
  return {
    version: 2,
    currency: 'USD',
    costType: 'recorded_estimated_or_verified_not_sent',
    attemptCountMeaning: 'ledger_records_including_verified_not_sent_not_model_call_count',
    attemptCount: unique.size,
    generatedAt: report.generatedAt,
    runId: report.run.id,
    productName: report.run.name,
    taskCounts: report.taskCounts,
    planningUsage: report.planningUsage?.map((planning) => ({ ...planning, usage: planning.usage.map(exportAttempt) })),
    turns: Array.from(unique.values(), exportAttempt),
    usage: {
      ...report.usageSummary,
      unassignedUsage: report.usageSummary.unassignedUsage?.map(exportAttempt),
      costEst: report.usageSummary.costUnknown ? null : (report.usageSummary.costEst ?? null),
      unknownReason:
        report.usageSummary.costUnknown || report.usageSummary.costEst == null
          ? 'missing_usage_or_model_pricing'
          : null,
      tasks: report.usageSummary.tasks.map((task) => ({
        ...task,
        attemptCount: task.attemptCount ?? task.turnCount,
        costEst: task.costEst ?? null,
      })),
    },
  };
}
