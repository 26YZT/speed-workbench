/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useState } from 'react';
import { Alert, Button, Input, Spin, Tag } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import type { ProductFactoryReview } from '../types';
import styles from '../ProductFactory.module.css';

type Props = {
  review?: ProductFactoryReview;
  loading: boolean;
  busy: boolean;
  error?: string;
  onReload: () => void;
  onApprove: () => Promise<void>;
  onRequestChanges: (feedback: string) => Promise<void>;
  onContinue: () => Promise<void>;
};

const ReviewPanel: React.FC<Props> = ({
  review,
  loading,
  busy,
  error,
  onReload,
  onApprove,
  onRequestChanges,
  onContinue,
}) => {
  const { t } = useTranslation();
  const [feedback, setFeedback] = useState('');
  if (loading)
    return (
      <section className={styles.reviewPanel}>
        <Spin />
      </section>
    );
  if (error || !review)
    return (
      <section className={styles.reviewPanel} role='alert'>
        <Alert type='error' content={error ?? t('productFactory.review.loadError')} />
        <Button className='mt-12px' onClick={onReload} disabled={busy}>
          {t('productFactory.review.reload')}
        </Button>
      </section>
    );
  const taskInReview = review.task.status === 'in_review';
  const canContinue = Boolean(review.nextTask) && review.task.status === 'completed';
  const canResume = review.task.status === 'in_progress' && review.execution?.state === 'enqueued';
  const unknown = review.usageSummary.costUnknown || review.usageSummary.costEst == null;
  return (
    <section className={styles.reviewPanel} aria-label={t('productFactory.review.title')}>
      <div className='flex flex-wrap items-start justify-between gap-12px'>
        <div>
          <h2 className='m-0 text-18px font-650 text-t-primary'>{t('productFactory.review.title')}</h2>
          <p className='m-0 mt-6px text-13px text-t-secondary'>{t('productFactory.review.description')}</p>
        </div>
        <div className='flex items-center gap-8px'>
          <Tag color={taskInReview ? 'orange' : review.task.status === 'completed' ? 'green' : 'arcoblue'}>
            {t(`productFactory.review.status.${review.task.status}`, { defaultValue: review.task.status })}
          </Tag>
          <Button onClick={onReload} disabled={busy}>
            {t('productFactory.review.reload')}
          </Button>
        </div>
      </div>
      <h3 className='mt-16px mb-0 text-16px font-600 text-t-primary'>{review.task.subject}</h3>
      {review.task.description && (
        <p className='m-0 mt-6px whitespace-pre-wrap text-13px text-t-secondary'>{review.task.description}</p>
      )}
      <div className='mt-12px rounded-8px bg-2 p-12px'>
        <div className='text-12px font-600 text-t-secondary'>{t('productFactory.review.workspace')}</div>
        <code className='mt-4px block break-all text-12px'>{review.workspace.path}</code>
      </div>
      <div className='mt-16px'>
        <h3 className='m-0 text-14px font-600 text-t-primary'>{t('productFactory.review.acceptanceCriteria')}</h3>
        <ul className='m-0 mt-8px pl-18px text-13px text-t-secondary'>
          {review.acceptanceCriteria.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
      <div className='mt-16px grid grid-cols-1 gap-8px sm:grid-cols-3'>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.inputTokens')}</div>
          <strong>{review.usageSummary.inputTokens}</strong>
        </div>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.outputTokens')}</div>
          <strong>{review.usageSummary.outputTokens}</strong>
        </div>
        <div className='rounded-8px bg-2 p-10px'>
          <div className='text-11px text-t-tertiary'>{t('productFactory.review.cost')}</div>
          <strong>
            {unknown ? t('productFactory.review.unknown') : `$${review.usageSummary.costEst!.toFixed(4)}`}
          </strong>
        </div>
      </div>
      {taskInReview ? (
        <div className='mt-18px flex flex-col gap-10px'>
          <Input.TextArea
            value={feedback}
            onChange={setFeedback}
            placeholder={t('productFactory.review.feedbackPlaceholder')}
            autoSize={{ minRows: 3, maxRows: 8 }}
            aria-label={t('productFactory.review.feedback')}
          />
          <div className='flex flex-wrap gap-8px'>
            <Button type='primary' loading={busy} disabled={busy} onClick={() => void onApprove()}>
              {t('productFactory.review.approve')}
            </Button>
            <Button
              status='warning'
              loading={busy}
              disabled={busy || !feedback.trim()}
              onClick={() => void onRequestChanges(feedback)}
            >
              {t('productFactory.review.requestChanges')}
            </Button>
          </div>
        </div>
      ) : canContinue ? (
        <div className='mt-18px flex flex-col gap-10px'>
          <Alert type='success' content={t('productFactory.review.approvedNext')} />
          <Button type='primary' loading={busy} disabled={busy} onClick={() => void onContinue()}>
            {t('productFactory.review.continue')}
          </Button>
        </div>
      ) : canResume ? (
        <div className='mt-18px flex flex-col gap-10px'>
          <Alert type='info' content={t('productFactory.review.resumeHint')} />
          <Button loading={busy} disabled={busy} onClick={() => void onContinue()}>
            {t('productFactory.review.resume')}
          </Button>
        </div>
      ) : review.execution?.state === 'uncertain' || review.execution?.state === 'pending' ? (
        <Alert className='mt-18px' type='warning' content={t('productFactory.execution.uncertain')} />
      ) : review.run.status === 'completed' ? (
        <Alert className='mt-18px' type='info' content={t('productFactory.review.productComplete')} />
      ) : null}
    </section>
  );
};
export default ReviewPanel;
