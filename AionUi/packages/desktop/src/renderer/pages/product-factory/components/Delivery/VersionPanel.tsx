/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useEffect, useState } from 'react';
import { Alert, Button, Card, Collapse, Descriptions, Space, Tag } from '@arco-design/web-react';
import { Refresh } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { ProductFactoryRun, ProductVersion, SourceManifest } from '../../types';
import { ProductFactoryHttpError } from '../../client';
import { getSourceManifest } from './versionClient';
import { useVersions } from './useVersions';
import ManifestReview from './ManifestReview';
import IterationDialog from './IterationDialog';

type Props = {
  run: ProductFactoryRun;
  onOpenRun: (runId: string) => void;
  onPreparationBlocked: (blocked: boolean) => void;
};

export default function VersionPanel({ run, onOpenRun, onPreparationBlocked }: Props) {
  const { t } = useTranslation();
  const versions = useVersions(run.id);
  const [source, setSource] = useState<SourceManifest>();
  const [readingSource, setReadingSource] = useState(false);
  const [sourceError, setSourceError] = useState<string>();
  const [iteration, setIteration] = useState<ProductVersion>();
  const [selectedVersion, setSelectedVersion] = useState<string>();
  const [activatedPath, setActivatedPath] = useState<string>();
  const current = versions.history?.versions.find((version) => version.id === versions.history?.currentVersionId);
  const blocked = current?.state === 'copying' || current?.state === 'failed';
  const canWrite = !versions.loading && !versions.busy && !versions.readFailed && !versions.pending;
  const pendingOperation = versions.history?.operations.find(
    (operation) => operation.idempotencyKey === versions.pending?.input.idempotency_key
  );
  const selected =
    versions.history?.versions.find((version) => version.id === selectedVersion) ??
    current ??
    versions.history?.versions[0];

  useEffect(() => {
    onPreparationBlocked(blocked);
  }, [blocked, onPreparationBlocked]);
  useEffect(() => () => onPreparationBlocked(false), [onPreparationBlocked]);
  useEffect(() => {
    const recovered = versions.history?.operations.find(
      (operation) => operation.idempotencyKey === versions.submittedKey && operation.state === 'complete'
    );
    if (recovered) {
      setSource(undefined);
      setIteration(undefined);
      setSelectedVersion(recovered.versionId);
      versions.acknowledgeReceipt();
    }
  }, [versions.history, versions.submittedKey, versions.acknowledgeReceipt]);
  const reviewManifest = async () => {
    setReadingSource(true);
    setSourceError(undefined);
    try {
      setSource(await getSourceManifest(run.id));
    } catch (caught) {
      setSourceError(
        caught instanceof ProductFactoryHttpError
          ? (caught.code ?? 'VERSION_SOURCE_UNAVAILABLE')
          : 'VERSION_SOURCE_UNAVAILABLE'
      );
    } finally {
      setReadingSource(false);
    }
  };

  return (
    <Card title={t('productFactory.versions.title')}>
      <p className='text-13px text-t-secondary'>{t('productFactory.versions.scopeHint')}</p>
      <Space wrap>
        <Button icon={<Refresh />} loading={versions.loading} onClick={() => void versions.refresh()}>
          {t('productFactory.versions.refresh')}
        </Button>
        <Button
          type='primary'
          loading={readingSource}
          disabled={!canWrite || readingSource || run.status !== 'completed' || current?.state === 'sealed'}
          onClick={() => void reviewManifest()}
        >
          {t('productFactory.versions.reviewSource')}
        </Button>
      </Space>
      {run.status !== 'completed' && (
        <p className='text-13px text-t-secondary'>{t('productFactory.versions.completeBeforeSnapshot')}</p>
      )}
      {versions.error && (
        <Alert className='mt-12px' type='error' content={t(`productFactory.versions.${versions.error}`)} />
      )}
      {(sourceError || versions.errorCode) && (
        <Alert
          className='mt-12px'
          type='error'
          content={t('productFactory.versions.errorCode', { code: sourceError ?? versions.errorCode })}
        />
      )}
      {blocked && (
        <Alert
          className='mt-12px'
          type='warning'
          content={t(
            current?.state === 'copying' ? 'productFactory.versions.copyingHint' : 'productFactory.versions.failedHint'
          )}
        />
      )}
      {versions.pending && (
        <div className='mt-12px'>
          <Alert type='warning' content={t('productFactory.versions.pendingHint')} />
          <Space className='mt-8px' wrap>
            <Button
              disabled={
                versions.busy ||
                versions.readFailed ||
                Boolean(pendingOperation && ['reserved', 'copying', 'failed'].includes(pendingOperation.state))
              }
              onClick={async () => {
                const receipt = await versions.retryPending();
                if (receipt?.operation.state === 'complete') {
                  setSource(undefined);
                  setIteration(undefined);
                  if (receipt.run) onOpenRun(receipt.run.id);
                }
              }}
            >
              {t('productFactory.versions.retrySameRequest')}
            </Button>
            <Button
              disabled={versions.busy}
              onClick={() => {
                versions.newRequest();
                setSource(undefined);
              }}
            >
              {t('productFactory.versions.newRequest')}
            </Button>
          </Space>
        </div>
      )}
      {!versions.loading && !versions.history?.product && <p>{t('productFactory.versions.unregistered')}</p>}
      {versions.history?.product && (
        <p className='text-13px text-t-secondary'>
          {t('productFactory.versions.productRevision', { revision: versions.history.product.revision })}
        </p>
      )}
      <div className='mt-14px flex flex-col gap-10px'>
        {versions.history?.versions.map((version) => (
          <div key={version.id} className='rounded-8px border border-border-2 p-12px'>
            <Space wrap>
              <strong>{t('productFactory.versions.number', { version: version.versionNo })}</strong>
              <Tag>{t(`productFactory.versions.states.${version.state}`)}</Tag>
              {version.id === versions.history?.product?.activeVersionId && (
                <Tag color='green'>{t('productFactory.versions.active')}</Tag>
              )}
              {version.id === versions.history?.currentVersionId && (
                <Tag>{t('productFactory.versions.currentRun')}</Tag>
              )}
            </Space>
            {version.parentVersionId && (
              <p className='text-13px text-t-secondary'>
                {t('productFactory.versions.parent', {
                  version:
                    versions.history?.versions.find((item) => item.id === version.parentVersionId)?.versionNo ??
                    version.parentVersionId,
                })}
              </p>
            )}
            {version.changeRequest && <p className='whitespace-pre-wrap'>{version.changeRequest}</p>}
            {version.errorCode && (
              <Alert
                className='mt-8px'
                type='error'
                content={t('productFactory.versions.errorCode', { code: version.errorCode })}
              />
            )}
            <Space className='mt-10px' wrap>
              <Button onClick={() => setSelectedVersion(version.id)}>{t('productFactory.versions.details')}</Button>
              <Button disabled={!canWrite || version.state !== 'sealed'} onClick={() => setIteration(version)}>
                {t('productFactory.versions.iterate')}
              </Button>
              <Button
                disabled={
                  !canWrite || version.state !== 'sealed' || version.id === versions.history?.product?.activeVersionId
                }
                onClick={async () => {
                  const receipt = await versions.activate(version.id, versions.history!.product!.revision);
                  if (receipt) setActivatedPath(receipt.workspacePath);
                }}
              >
                {t('productFactory.versions.activate')}
              </Button>
              {version.runId !== run.id && (
                <Button onClick={() => onOpenRun(version.runId)}>{t('productFactory.versions.openRun')}</Button>
              )}
            </Space>
          </div>
        ))}
      </div>
      {selected && (
        <Descriptions
          className='mt-16px'
          size='small'
          column={1}
          data={[
            { label: t('productFactory.versions.run'), value: selected.runId },
            {
              label: t('productFactory.versions.path'),
              value: selected.snapshotPath ?? t('productFactory.versions.noSnapshot'),
            },
            { label: t('productFactory.versions.fileCount'), value: selected.manifest?.files.length ?? 0 },
          ]}
        />
      )}
      {selected?.manifest && (
        <Collapse className='mt-12px'>
          <Collapse.Item name='manifest' header={t('productFactory.versions.savedManifest')}>
            <ul className='max-h-240px overflow-auto pl-20px'>
              {selected.manifest.files.map((file) => (
                <li key={file.path}>
                  <code>{file.path}</code> · {file.sizeBytes} B
                </li>
              ))}
            </ul>
          </Collapse.Item>
        </Collapse>
      )}
      {activatedPath && (
        <Alert
          className='mt-12px'
          type='success'
          content={t('productFactory.versions.activationResult', { path: activatedPath })}
        />
      )}
      {source && (
        <ManifestReview
          source={source}
          busy={versions.busy}
          pending={Boolean(versions.pending)}
          onClose={() => setSource(undefined)}
          onSubmit={async (files) => {
            const receipt = await versions.submit({
              kind: 'snapshot',
              input: {
                expected_plan_revision: source.planRevision,
                idempotency_key: window.crypto.randomUUID(),
                files,
              },
            });
            if (!receipt) throw new Error('VERSION_SNAPSHOT_UNCONFIRMED');
            setSource(undefined);
          }}
        />
      )}
      {iteration && (
        <IterationDialog
          version={iteration}
          busy={versions.busy}
          pending={Boolean(versions.pending)}
          onClose={() => setIteration(undefined)}
          onSubmit={async (changeRequest, budget) => {
            const receipt = await versions.submit({
              kind: 'iterate',
              input: {
                source_version_id: iteration.id,
                expected_product_revision: versions.history!.product!.revision,
                idempotency_key: window.crypto.randomUUID(),
                change_request: changeRequest,
                ...(budget == null ? {} : { budget_usd: budget }),
              },
            });
            if (!receipt) throw new Error('VERSION_ITERATION_UNCONFIRMED');
            setIteration(undefined);
            if (receipt.run) onOpenRun(receipt.run.id);
          }}
        />
      )}
    </Card>
  );
}
