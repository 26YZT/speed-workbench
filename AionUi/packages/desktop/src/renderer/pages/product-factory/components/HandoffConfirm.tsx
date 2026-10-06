/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import React, { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { Alert, Button, Spin } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import { handoffProductFactoryRun, ProductFactoryHttpError } from '../client';
import type { ProductFactoryRun, TaskDraftArtifact } from '../types';

// Keep the existing picker/context dependencies out of the read-only preview.
const TeamCreateModal = lazy(() => import('@renderer/pages/team/components/TeamCreateModal'));

type Props = { run: ProductFactoryRun; onResult: (run: ProductFactoryRun) => void };

const HandoffConfirm: React.FC<Props> = ({ run, onResult }) => {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);
  const [error, setError] = useState<string>();
  const active = useRef(true);
  const resultCallback = useRef(onResult);
  resultCallback.current = onResult;
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  return (
    <div className='mt-16px'>
      <p>{t('productFactory.handoff.execution.description')}</p>
      {error && <Alert type='error' content={error} className='mb-12px' />}
      <Button
        type='primary'
        onClick={() => {
          setError(undefined);
          setVisible(true);
        }}
      >
        {t('productFactory.handoff.execution.configure')}
      </Button>
      {visible && (
        <Suspense fallback={<Spin />}>
          <TeamCreateModal
            visible
            onClose={() => setVisible(false)}
            onCreated={() => undefined}
            creationOverride={{
              name: run.name,
              workspace: run.workspacePath,
              title: t('productFactory.handoff.execution.configure'),
              subtitle: t('productFactory.handoff.execution.description'),
              confirmLabel: t('productFactory.handoff.execution.confirm'),
              submit: async (agents) => {
                try {
                  const updated = await handoffProductFactoryRun(
                    run.id,
                    (run.taskDraft as TaskDraftArtifact).revision,
                    agents
                  );
                  if (active.current) resultCallback.current(updated);
                } catch (caught) {
                  const code = caught instanceof ProductFactoryHttpError ? caught.code : undefined;
                  const message =
                    code === 'PRODUCT_FACTORY_REVISION_CONFLICT'
                      ? t('productFactory.handoff.execution.revisionError')
                      : code === 'PRODUCT_FACTORY_HANDOFF_BLOCKED'
                        ? t('productFactory.handoff.execution.blockedError')
                        : code === 'PRODUCT_FACTORY_TEAM_PREPARATION_FAILED'
                          ? t('productFactory.handoff.execution.preparationError')
                          : t('productFactory.handoff.execution.requestError');
                  if (active.current) setError(message);
                  throw new Error(message, { cause: caught });
                }
              },
            }}
          />
        </Suspense>
      )}
    </div>
  );
};

export default HandoffConfirm;
