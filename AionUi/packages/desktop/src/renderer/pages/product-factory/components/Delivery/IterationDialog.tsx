/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useState } from 'react';
import { Alert, Button, Input, InputNumber, Modal } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import type { ProductVersion } from '../../types';

type Props = {
  version: ProductVersion;
  busy: boolean;
  pending: boolean;
  onClose: () => void;
  onSubmit: (changeRequest: string, budget?: number) => Promise<void>;
};

export default function IterationDialog({ version, busy, pending, onClose, onSubmit }: Props) {
  const { t } = useTranslation();
  const [change, setChange] = useState('');
  const [budget, setBudget] = useState<number>();
  const [error, setError] = useState(false);
  const valid = change.trim().length > 0 && (budget == null || (Number.isFinite(budget) && budget > 0));
  return (
    <Modal
      visible
      title={t('productFactory.versions.iterationTitle', { version: version.versionNo })}
      onCancel={onClose}
      footer={null}
      style={{ width: 620 }}
    >
      <p>{t('productFactory.versions.iterationHint')}</p>
      <label className='mb-14px block'>
        <span className='mb-8px block'>{t('productFactory.versions.changeRequest')}</span>
        <Input.TextArea
          value={change}
          disabled={busy || pending}
          onChange={setChange}
          autoSize={{ minRows: 4, maxRows: 10 }}
          aria-label={t('productFactory.versions.changeRequest')}
        />
      </label>
      <label className='mb-14px block'>
        <span className='mb-8px block'>{t('productFactory.versions.iterationBudget')}</span>
        <InputNumber
          value={budget}
          onChange={setBudget}
          disabled={busy || pending}
          min={0.01}
          precision={2}
          style={{ width: '100%' }}
          placeholder={t('productFactory.form.budgetPlaceholder')}
          aria-label={t('productFactory.versions.iterationBudget')}
        />
      </label>
      <p className='text-13px text-t-secondary'>{t('productFactory.versions.independentBudget')}</p>
      {error && <Alert type='error' content={t('productFactory.versions.operationError')} />}
      <div className='mt-16px flex justify-end gap-8px'>
        <Button disabled={busy} onClick={onClose}>
          {t('productFactory.versions.close')}
        </Button>
        <Button
          type='primary'
          loading={busy}
          disabled={!valid || busy || pending}
          onClick={async () => {
            setError(false);
            try {
              await onSubmit(change.trim(), budget);
            } catch {
              setError(true);
            }
          }}
        >
          {t('productFactory.versions.createIteration')}
        </Button>
      </div>
    </Modal>
  );
}
