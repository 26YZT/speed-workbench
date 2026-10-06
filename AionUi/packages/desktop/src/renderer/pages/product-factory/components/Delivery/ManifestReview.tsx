/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useEffect, useState } from 'react';
import { Alert, Button, Checkbox, Modal } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import type { SourceManifest } from '../../types';

type Props = {
  source: SourceManifest;
  busy: boolean;
  pending: boolean;
  onClose: () => void;
  onSubmit: (files: Array<{ path: string; sha256: string }>) => Promise<void>;
};

export default function ManifestReview({ source, busy, pending, onClose, onSubmit }: Props) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState(() => new Set(source.manifest.files.map((file) => file.path)));
  const [error, setError] = useState(false);
  useEffect(() => {
    setSelected(new Set(source.manifest.files.map((file) => file.path)));
  }, [source]);
  const valid = selected.size > 0 && selected.has('START.md') && selected.has('ACCEPTANCE.md');
  return (
    <Modal
      visible
      title={t('productFactory.versions.manifestTitle')}
      onCancel={onClose}
      footer={null}
      style={{ width: 680 }}
    >
      <p>{t('productFactory.versions.manifestHint')}</p>
      <p className='text-13px text-t-secondary'>
        {t('productFactory.versions.excluded', { count: source.excludedCount })}
      </p>
      <div className='max-h-380px overflow-auto rounded-8px border border-border-2 p-12px'>
        {source.manifest.files.map((file) => (
          <div key={file.path} className='mb-12px'>
            <Checkbox
              checked={selected.has(file.path)}
              disabled={busy || pending}
              onChange={(checked) =>
                setSelected((current) => {
                  const next = new Set(current);
                  if (checked) next.add(file.path);
                  else next.delete(file.path);
                  return next;
                })
              }
            >
              {file.path}
            </Checkbox>
            <div className='ms-24px text-12px text-t-secondary'>
              <span>{file.sizeBytes} B</span>
              <code className='ms-8px break-all'>{file.sha256}</code>
            </div>
          </div>
        ))}
      </div>
      {!valid && <Alert className='mt-12px' type='warning' content={t('productFactory.versions.requiredFiles')} />}
      {error && <Alert className='mt-12px' type='error' content={t('productFactory.versions.operationError')} />}
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
              await onSubmit(
                source.manifest.files
                  .filter((file) => selected.has(file.path))
                  .map(({ path, sha256 }) => ({ path, sha256 }))
              );
            } catch {
              setError(true);
            }
          }}
        >
          {t('productFactory.versions.saveSnapshot')}
        </Button>
      </div>
    </Modal>
  );
}
