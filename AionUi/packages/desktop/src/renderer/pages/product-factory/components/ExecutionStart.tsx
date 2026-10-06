/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { useEffect, useRef, useState } from 'react';
import { Alert, Button, Modal, Spin } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import { getProductFactoryExecution, ProductFactoryHttpError, startProductFactoryRun } from '../client';
import type { FactoryExecution, ProductFactoryRun, TaskDraftArtifact } from '../types';

const ExecutionStart: React.FC<{ run: ProductFactoryRun }> = ({ run }) => {
  const { t } = useTranslation();
  const [execution, setExecution] = useState<FactoryExecution>();
  const [readable, setReadable] = useState(false);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string>();
  const active = useRef(true);
  const busy = useRef(false);

  const read = async () => {
    setLoading(true);
    setReadable(false);
    try {
      const receipt = await getProductFactoryExecution(run.id);
      if (receipt && receipt.teamId !== run.teamId) throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
      if (active.current) {
        setExecution(receipt);
        setReadable(true);
      }
    } catch {
      if (active.current) setError(t('productFactory.execution.readError'));
    } finally {
      if (active.current) setLoading(false);
    }
  };
  useEffect(() => {
    active.current = true;
    void read();
    return () => {
      active.current = false;
    };
  }, [run.id]);

  const start = async () => {
    if (busy.current || !readable || execution || run.status !== 'handed_off') return;
    busy.current = true;
    setStarting(true);
    setError(undefined);
    try {
      const receipt = await startProductFactoryRun(run.id, (run.taskDraft as TaskDraftArtifact).revision);
      if (receipt.teamId !== run.teamId) throw new Error('PRODUCT_FACTORY_INVALID_EXECUTION_RESPONSE');
      if (active.current) setExecution(receipt);
    } catch (caught) {
      const code = caught instanceof ProductFactoryHttpError ? caught.code : undefined;
      if (active.current) {
        setError(
          t(
            code === 'PRODUCT_FACTORY_REVISION_CONFLICT'
              ? 'productFactory.execution.revisionError'
              : code === 'PRODUCT_FACTORY_COST_UNKNOWN'
                ? 'productFactory.execution.costUnknown'
                : code === 'PRODUCT_FACTORY_EXECUTION_BLOCKED'
                  ? 'productFactory.execution.blocked'
                  : 'productFactory.execution.requestError'
          )
        );
        // The POST may have committed despite a lost response. Read once, never resend.
        await read();
      }
    } finally {
      busy.current = false;
      if (active.current) {
        setStarting(false);
        setConfirming(false);
      }
    }
  };
  const stateCopy = {
    pending: 'productFactory.execution.pending',
    enqueued: 'productFactory.execution.enqueued',
    uncertain: 'productFactory.execution.uncertain',
  } as const;
  return (
    <section className='mt-18px' data-testid='factory-execution'>
      <h2>{t('productFactory.execution.title')}</h2>
      <p>{t('productFactory.execution.description')}</p>
      {error && <Alert type='error' content={error} className='mb-12px' />}
      {loading ? (
        <Spin />
      ) : !readable ? (
        <Button
          onClick={() => {
            setError(undefined);
            void read();
          }}
        >
          {t('productFactory.workspace.retry')}
        </Button>
      ) : execution ? (
        <>
          <Alert type={execution.state === 'enqueued' ? 'info' : 'warning'} content={t(stateCopy[execution.state])} />
          <p>
            {t('productFactory.execution.taskId')}: {execution.taskId}
          </p>
          {execution.messageId && (
            <p>
              {t('productFactory.execution.messageId')}: {execution.messageId}
            </p>
          )}
          <p>{t('productFactory.execution.disclaimer')}</p>
        </>
      ) : run.status === 'handed_off' ? (
        <Button
          type='primary'
          disabled={starting || !(run.taskDraft as TaskDraftArtifact | undefined)?.confirmed}
          onClick={() => setConfirming(true)}
        >
          {t('productFactory.execution.start')}
        </Button>
      ) : null}
      <Modal
        visible={confirming}
        title={t('productFactory.execution.confirmTitle')}
        okText={t('productFactory.execution.confirm')}
        cancelText={t('common.cancel')}
        confirmLoading={starting}
        onOk={start}
        onCancel={() => {
          if (!starting) setConfirming(false);
        }}
      >
        <p>{t('productFactory.execution.confirmDescription')}</p>
        <p>{run.workspacePath}</p>
      </Modal>
    </section>
  );
};
export default ExecutionStart;
