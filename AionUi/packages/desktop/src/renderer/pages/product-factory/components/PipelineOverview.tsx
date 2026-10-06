/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { useTranslation } from 'react-i18next';

const STEP_KEYS = ['idea', 'interview', 'blueprint', 'tasks', 'execution', 'review', 'delivery'] as const;

const PipelineOverview: React.FC = () => {
  const { t } = useTranslation();

  return (
    <section
      aria-labelledby='product-factory-pipeline-title'
      className='rounded-12px border border-border-2 bg-bg-2 p-18px'
    >
      <h2 id='product-factory-pipeline-title' className='m-0 text-14px font-600 text-t-primary'>
        {t('productFactory.pipeline.title')}
      </h2>
      <ol className='m-0 mt-16px grid list-none grid-cols-7 gap-8px p-0 max-980px:grid-cols-4 max-640px:grid-cols-2'>
        {STEP_KEYS.map((key, index) => (
          <li key={key} className='min-w-0 rounded-8px border border-border-2 bg-fill-1 px-10px py-12px'>
            <div className='mb-8px flex size-24px items-center justify-center rounded-full bg-fill-3 text-12px font-600 text-t-secondary'>
              {index + 1}
            </div>
            <span className='text-13px font-500 text-t-primary'>{t(`productFactory.pipeline.steps.${key}`)}</span>
          </li>
        ))}
      </ol>
    </section>
  );
};

export default PipelineOverview;
