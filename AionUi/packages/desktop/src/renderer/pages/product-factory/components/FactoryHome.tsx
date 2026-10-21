/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Button, Tag } from '@arco-design/web-react';
import { ArrowRight, Plus } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { ProductFactoryRun, ProductIdeaDraft } from '../types';
import PipelineOverview from './PipelineOverview';
import { factoryRunStatusKey } from '../runStatus';
import styles from '../ProductFactory.module.css';
import { hasProductIdeaDraft } from '../draftStore';

type FactoryHomeProps = {
  draft: ProductIdeaDraft;
  runs: ProductFactoryRun[];
  loadingRuns: boolean;
  apiUnavailable: boolean;
  onRetry: () => void;
  onCreate: () => void;
  onContinue: () => void;
  onOpenRun?: (runId: string) => void;
};

const FactoryHome: React.FC<FactoryHomeProps> = ({
  draft,
  runs,
  loadingRuns,
  apiUnavailable,
  onRetry,
  onCreate,
  onContinue,
  onOpenRun,
}) => {
  const { t } = useTranslation();
  const hasDraft = hasProductIdeaDraft(draft);

  return (
    <div className='flex flex-col gap-20px'>
      <header className={styles.factoryHeader}>
        <div className='max-w-720px'>
          <div className='mb-8px text-12px font-600 uppercase tracking-1px text-t-secondary'>
            {t('productFactory.home.eyebrow')}
          </div>
          <h1 className='m-0 text-28px font-650 leading-38px text-t-primary max-640px:text-24px'>
            {t('productFactory.home.title')}
          </h1>
          <p className='m-0 mt-10px text-14px leading-22px text-t-secondary'>{t('productFactory.home.description')}</p>
        </div>
        <Button type='primary' size='large' icon={<Plus size={17} />} onClick={onCreate}>
          {t('productFactory.home.createProduct')}
        </Button>
      </header>

      <PipelineOverview />

      {loadingRuns && <p className='m-0 text-13px text-t-secondary'>{t('common.loading')}</p>}
      {apiUnavailable && !loadingRuns && (
        <div className='flex items-center justify-between gap-12px rounded-8px border border-warning-3 bg-warning-1 px-14px py-10px text-13px text-warning-7'>
          <span>{t('login.errors.networkError')}</span>
          <Button size='small' type='text' onClick={onRetry}>
            {t('common.retry')}
          </Button>
        </div>
      )}

      {hasDraft && (
        <section data-testid='product-idea-draft-card' className='rounded-12px border border-border-2 bg-bg-2 p-20px'>
          <div className={styles.factoryCardHeader}>
            <div className='min-w-0'>
              <Tag color='arcoblue' size='small'>
                {t('productFactory.home.localDraft')}
              </Tag>
              <h2 className='m-0 mt-12px text-18px font-600 text-t-primary'>
                {draft.name || t('productFactory.status.draft')}
              </h2>
              <p className='m-0 mt-8px line-clamp-2 text-13px leading-20px text-t-secondary'>{draft.idea}</p>
            </div>
            <Button type='secondary' icon={<ArrowRight size={16} />} onClick={onContinue}>
              {t('productFactory.home.continueDraft')}
            </Button>
          </div>
        </section>
      )}
      {runs.length > 0 && (
        <section aria-labelledby='factory-products-title'>
          <h2 id='factory-products-title' className='m-0 mb-12px text-16px font-600 text-t-primary'>
            {t('productFactory.home.productsTitle')}
          </h2>
          <div className='flex flex-col gap-12px'>
            {runs.map((run) => {
              const action = t(`productFactory.home.actions.${run.status}`);
              const updated = new Date(Number.isFinite(run.updatedAt) ? run.updatedAt : run.createdAt || 0);
              return (
                <article
                  key={run.id}
                  data-testid={`factory-run-${run.id}`}
                  className='rounded-12px border border-border-2 bg-bg-2 p-20px'
                >
                  <div className={styles.factoryCardHeader}>
                    <div className='min-w-0'>
                      <Tag color='arcoblue' size='small'>
                        {t(factoryRunStatusKey(run.status))}
                      </Tag>
                      <h3 className='m-0 mt-12px break-words text-18px font-600 text-t-primary'>{run.name}</h3>
                      <p className='m-0 mt-8px line-clamp-2 text-13px leading-20px text-t-secondary'>{run.idea}</p>
                      <time className='mt-10px block text-12px text-t-secondary' dateTime={updated.toISOString()}>
                        {t('productFactory.home.updatedAt', { time: updated.toLocaleString() })}
                      </time>
                    </div>
                    <Button
                      type='secondary'
                      aria-label={`${action}: ${run.name}`}
                      icon={<ArrowRight size={16} />}
                      onClick={() => (onOpenRun ? onOpenRun(run.id) : onContinue())}
                    >
                      {action}
                    </Button>
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      )}
      {!runs.length && !hasDraft && (
        <section className='flex min-h-190px flex-col items-center justify-center rounded-12px border border-dashed border-border-3 bg-fill-1 px-24px py-32px text-center'>
          <h2 className='m-0 text-17px font-600 text-t-primary'>{t('productFactory.home.emptyTitle')}</h2>
          <p className='m-0 mt-8px max-w-520px text-13px leading-20px text-t-secondary'>
            {t('productFactory.home.emptyDescription')}
          </p>
          <Button className='mt-18px' type='outline' icon={<Plus size={16} />} onClick={onCreate}>
            {t('productFactory.home.createProduct')}
          </Button>
        </section>
      )}
    </div>
  );
};

export default FactoryHome;
