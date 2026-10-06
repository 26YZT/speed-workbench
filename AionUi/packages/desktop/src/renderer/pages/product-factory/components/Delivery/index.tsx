/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useEffect, useState } from 'react';
import { Alert, Button, Tag } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import { getProductFactoryCostReport, repriceProductFactoryCosts } from '../../client';
import type { ProductFactoryDelivery } from '../../types';
import styles from '../../ProductFactory.module.css';

import { buildCostReportExport } from './costReport';

type Props = { delivery: ProductFactoryDelivery };

const DeliveryPanel: React.FC<Props> = ({ delivery: initialDelivery }) => {
  const { t } = useTranslation();
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState(false);
  const [updatedDelivery, setUpdatedDelivery] = useState<ProductFactoryDelivery>();
  const [repricing, setRepricing] = useState(false);
  const [repriceError, setRepriceError] = useState(false);
  const [repriceCounts, setRepriceCounts] = useState<{ updated: number; skipped: number }>();
  useEffect(() => {
    setUpdatedDelivery(undefined);
    setRepriceCounts(undefined);
    setRepriceError(false);
    setExportError(false);
  }, [initialDelivery]);
  const delivery = updatedDelivery ?? initialDelivery;
  const reprice = async () => {
    if (repricing || exporting) return;
    setRepricing(true);
    setRepriceError(false);
    setRepriceCounts(undefined);
    try {
      const result = await repriceProductFactoryCosts(delivery.run.id);
      setUpdatedDelivery(result.delivery);
      setRepriceCounts({ updated: result.updatedCount, skipped: result.skippedCount });
    } catch {
      setRepriceError(true);
    } finally {
      setRepricing(false);
    }
  };
  const summary = delivery.usageSummary;
  const unknown = summary.costUnknown || summary.costEst == null;
  const cost = unknown ? t('productFactory.review.unknown') : `$${summary.costEst!.toFixed(4)}`;
  const exportReport = async () => {
    if (exporting || repricing) return;
    setExporting(true);
    setExportError(false);
    try {
      const report = await getProductFactoryCostReport(delivery.run.id);
      const content = buildCostReportExport(report);
      const { downloadTextContent } = await import('@renderer/utils/file/download');
      downloadTextContent(
        JSON.stringify(content, null, 2),
        `product-factory-${report.run.id}-cost.json`,
        'application/json'
      );
    } catch {
      setExportError(true);
    } finally {
      setExporting(false);
    }
  };
  return (
    <section className={styles.reviewPanel} aria-label={t('productFactory.pipeline.steps.delivery')}>
      <Alert
        type={delivery.ready ? 'success' : 'warning'}
        content={t(delivery.ready ? 'productFactory.delivery.ready' : 'productFactory.delivery.notReady')}
      />
      <p className='text-13px text-t-secondary'>{t('productFactory.delivery.verifyHint')}</p>
      <div className='mt-14px rounded-8px bg-2 p-12px'>
        <div className='text-12px font-600 text-t-secondary'>{t('productFactory.delivery.workspace')}</div>
        <code className='mt-4px block break-all text-12px'>{delivery.workspacePath}</code>
      </div>
      <h3 className='mt-16px text-14px font-600'>{t('productFactory.delivery.checksTitle')}</h3>
      <div className='flex flex-col gap-8px'>
        {delivery.checks.map((check) => (
          <div key={check.code} className='flex items-start justify-between gap-12px rounded-8px bg-2 p-10px text-13px'>
            <span>{t(`productFactory.delivery.checks.${check.code}`, { defaultValue: check.detail })}</span>
            <Tag color={check.passed ? 'green' : 'orange'}>
              {t(check.passed ? 'productFactory.delivery.passed' : 'productFactory.delivery.pending')}
            </Tag>
          </div>
        ))}
      </div>
      <div className='mt-14px grid grid-cols-1 gap-8px sm:grid-cols-3'>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.inputTokens')}</div>
          <strong>{summary.inputTokens}</strong>
        </div>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.outputTokens')}</div>
          <strong>{summary.outputTokens}</strong>
        </div>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.cost')}</div>
          <strong>{cost}</strong>
        </div>
      </div>
      {unknown && <Alert className='mt-12px' type='warning' content={t('productFactory.delivery.unknownCost')} />}
      {summary.budgetLimitUsd != null && (
        <p className='text-13px text-t-secondary'>
          {t('productFactory.delivery.budgetSummary', {
            limit: summary.budgetLimitUsd.toFixed(2),
            remaining:
              unknown || summary.budgetRemainingUsd == null
                ? t('productFactory.review.unknown')
                : summary.budgetRemainingUsd.toFixed(2),
          })}
        </p>
      )}
      <p className='text-12px text-t-secondary'>{t('productFactory.delivery.attemptCountHint')}</p>
      <h3 className='mt-16px text-14px font-600'>{t('productFactory.pipeline.steps.tasks')}</h3>
      <div className='flex flex-col gap-6px'>
        {summary.tasks.map((task) => (
          <div
            key={task.taskId}
            className='flex flex-wrap items-center justify-between gap-12px rounded-8px bg-2 p-8px text-12px'
          >
            <code className='break-all'>{task.taskId}</code>
            <span className='text-t-secondary'>
              {task.inputTokens} / {task.outputTokens} ·{' '}
              {task.costEst == null ? t('productFactory.review.unknown') : `$${task.costEst.toFixed(4)}`}
            </span>
          </div>
        ))}
      </div>
      <Button
        className='mt-16px'
        loading={exporting}
        disabled={exporting || repricing}
        onClick={() => void exportReport()}
      >
        {t('productFactory.delivery.exportCostReport')}
      </Button>
      {exportError && <Alert className='mt-12px' type='error' content={t('productFactory.delivery.exportError')} />}
      <Button
        className='mt-16px ml-8px'
        loading={repricing}
        disabled={repricing || exporting}
        onClick={() => void reprice()}
      >
        {t('productFactory.delivery.reprice')}
      </Button>
      <p className='text-12px text-t-secondary'>{t('productFactory.delivery.repriceHint')}</p>
      {repriceCounts && <Alert type='info' content={t('productFactory.delivery.repriceResult', repriceCounts)} />}
      {repriceError && <Alert type='error' content={t('productFactory.delivery.repriceError')} />}
    </section>
  );
};
export default DeliveryPanel;
