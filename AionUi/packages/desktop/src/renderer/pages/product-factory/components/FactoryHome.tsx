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

type FactoryHomeProps = {
  draft: ProductIdeaDraft;
  runs: ProductFactoryRun[];
  loadingRuns: boolean;
  apiUnavailable: boolean;
  onRetry: () => void;
  onCreate: () => void;
  onContinue: () => void;
};

const FactoryHome: React.FC<FactoryHomeProps> = ({
  draft,
  runs,
  loadingRuns,
  apiUnavailable,
  onRetry,
  onCreate,
  onContinue,
}) => {
  const { t } = useTranslation();
  const hasDraft = Boolean(draft.name || draft.idea);

  return (
    <div className='flex flex-col gap-20px'>
      <header className='flex items-start justify-between gap-20px max-720px:flex-col'>
        <div className='max-w-720px'>
          <div className='mb-8px text-12px font-600 uppercase tracking-1px text-brand-6'>
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

      {runs.length > 0 || hasDraft ? (
        <section className='rounded-12px border border-border-2 bg-bg-2 p-20px'>
          <div className='flex items-start justify-between gap-18px max-640px:flex-col'>
            <div className='min-w-0'>
              <Tag color='arcoblue' size='small'>
                {runs[0] ? runs[0].status : t('productFactory.status.draft')}
              </Tag>
              <h2 className='m-0 mt-12px truncate text-18px font-600 text-t-primary'>{draft.name}</h2>
              <p className='m-0 mt-8px line-clamp-2 text-13px leading-20px text-t-secondary'>{draft.idea}</p>
            </div>
            <Button type='secondary' icon={<ArrowRight size={16} />} onClick={onContinue}>
              {t('productFactory.home.continueDraft')}
            </Button>
          </div>
        </section>
      ) : (
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
