/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { lazy, Suspense, useEffect, useState } from 'react';
import { Alert, Button, Card, Descriptions, Select, Space, Spin, Tag } from '@arco-design/web-react';
import { Refresh } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { PlanningPhase, ProductFactoryRun } from '../../../types';
import { asRecord } from '../../../handoffPreview';
import CandidatePreview from './CandidatePreview';
import { isPlanningActive, usePlanning } from './usePlanning';

const TeamCreateModal = lazy(() => import('@renderer/pages/team/components/TeamCreateModal'));
const planningErrors = {
  USER_MODEL_SEND_ADMISSION_REJECTED: 'sendAdmissionRejected',
  PLANNING_UNCERTAIN_ATTEMPT: 'uncertainAttempt',
  PLANNING_RESTART_UNCERTAIN: 'uncertainAttempt',
  PLANNING_CONVERSATION_NOT_BOUND: 'conversationNotBound',
  PLANNING_ATTEMPT_NOT_RUNNING: 'attemptNotRunning',
  PLANNING_APP_TURN_NOT_BOUND: 'appTurnNotBound',
  PLANNING_CANCELLATION_UNCONFIRMED: 'cancellationUnconfirmed',
} as const;
type Props = {
  run: ProductFactoryRun;
  disabled: boolean;
  onApplied: (run: ProductFactoryRun) => void;
  onActiveChange: (active: boolean) => void;
};

export default function PlanningPanel({ run, disabled, onApplied, onActiveChange }: Props) {
  const { t } = useTranslation();
  const planning = usePlanning(run.id);
  const recommendedPhase: PlanningPhase = ['draft', 'interviewing'].includes(run.status)
    ? 'interview'
    : run.status === 'blueprint_ready' &&
        (asRecord(run.blueprint).version !== 2 || asRecord(run.blueprint).confirmed !== true)
      ? 'blueprint'
      : 'task_graph';
  const [phase, setPhase] = useState<PlanningPhase>(recommendedPhase);
  const [selectedId, setSelectedId] = useState<string>();
  const [pickerVisible, setPickerVisible] = useState(false);
  const [configurationError, setConfigurationError] = useState(false);
  const selected = planning.attempts.find((attempt) => attempt.id === selectedId) ?? planning.attempts[0];
  const uncertain = planning.attempts.some((attempt) => attempt.state === 'uncertain');
  const active = planning.hasActive;
  const canGenerate = !disabled && !planning.loading && !planning.busy && !planning.readFailed && !active && !uncertain;

  useEffect(() => {
    setPhase(recommendedPhase);
  }, [recommendedPhase]);
  useEffect(() => {
    onActiveChange(active || planning.busy);
  }, [active, planning.busy, onActiveChange]);
  useEffect(() => () => onActiveChange(false), [onActiveChange]);
  const summary = planning.report?.usageSummary;
  const explanation = planningErrors[(planning.errorCode ?? selected?.errorCode) as keyof typeof planningErrors];

  return (
    <Card title={t('productFactory.planning.title')}>
      <p className='text-13px text-t-secondary'>{t('productFactory.planning.description')}</p>
      <p className='text-13px text-t-secondary'>{t('productFactory.planning.savedInputHint')}</p>
      <Space wrap>
        <Select
          aria-label={t('productFactory.planning.phase')}
          value={phase}
          onChange={setPhase}
          disabled={planning.busy || active}
          style={{ minWidth: 180 }}
        >
          {(['interview', 'blueprint', 'task_graph'] as PlanningPhase[]).map((value) => (
            <Select.Option key={value} value={value}>
              {t(`productFactory.planning.phases.${value}`)}
            </Select.Option>
          ))}
        </Select>
        <Button
          type='primary'
          disabled={!canGenerate}
          onClick={() => {
            setConfigurationError(false);
            setPickerVisible(true);
          }}
        >
          {t('productFactory.planning.generate')}
        </Button>
        <Button icon={<Refresh />} loading={planning.loading} onClick={() => void planning.refresh()}>
          {t('productFactory.planning.refresh')}
        </Button>
      </Space>
      {planning.error && (
        <Alert
          className='mt-12px'
          type='error'
          content={t(`productFactory.planning.${planning.error as 'readError' | 'requestError'}`)}
        />
      )}
      {planning.errorCode && (
        <Alert
          className='mt-12px'
          type='error'
          content={t('productFactory.planning.operationCode', { code: planning.errorCode })}
        />
      )}
      {configurationError && (
        <Alert className='mt-12px' type='error' content={t('productFactory.planning.oneAssistant')} />
      )}
      {uncertain && <Alert className='mt-12px' type='warning' content={t('productFactory.planning.uncertainHint')} />}
      {explanation && (
        <Alert className='mt-12px' type='warning' content={t(`productFactory.planning.errors.${explanation}`)} />
      )}
      {planning.attempts.length > 0 && (
        <Select
          className='mt-16px w-full'
          aria-label={t('productFactory.planning.attempts')}
          value={selected?.id}
          onChange={setSelectedId}
          options={planning.attempts.map((attempt) => ({
            value: attempt.id,
            label: `${t(`productFactory.planning.phases.${attempt.phase}`)} · ${t(`productFactory.planning.states.${attempt.state}`)} · ${new Date(attempt.createdAt).toLocaleString()}`,
          }))}
        />
      )}
      {selected && (
        <div className='mt-16px'>
          <Space wrap>
            <Tag>{t(`productFactory.planning.states.${selected.state}`)}</Tag>
            <Tag>{t('productFactory.planning.modelSource')}</Tag>
          </Space>
          <Descriptions
            className='mt-12px'
            size='small'
            column={2}
            data={[
              { label: t('productFactory.planning.assistant'), value: selected.assistantId },
              {
                label: t('productFactory.planning.model'),
                value: selected.model || t('productFactory.planning.defaultModel'),
              },
              { label: t('productFactory.planning.revision'), value: selected.inputPlanRevision },
              {
                label: t('productFactory.planning.cost'),
                value:
                  summary && !summary.costUnknown && summary.costEst != null
                    ? `$${summary.costEst.toFixed(4)}`
                    : t('productFactory.review.unknown'),
              },
            ]}
          />
          {selected.errorCode && (
            <Alert
              className='mt-12px'
              type='error'
              content={t('productFactory.planning.failureCode', { code: selected.errorCode })}
            />
          )}
          <CandidatePreview attempt={selected} />
          <Space wrap className='mt-12px'>
            {selected.state === 'candidate_ready' && (
              <Button
                type='primary'
                disabled={
                  disabled ||
                  planning.busy ||
                  planning.readFailed ||
                  selected.inputPlanRevision !== (run.planRevision ?? 1)
                }
                onClick={async () => {
                  const updated = await planning.apply(selected, run.planRevision ?? 1);
                  if (updated) onApplied(updated);
                }}
              >
                {t('productFactory.planning.apply')}
              </Button>
            )}
            {isPlanningActive(selected) && (
              <Button disabled={planning.busy} onClick={() => void planning.cancel(selected)}>
                {t('productFactory.planning.cancel')}
              </Button>
            )}
            {['failed', 'cancelled'].includes(selected.state) && (
              <Button
                disabled={!canGenerate}
                onClick={() => {
                  setPhase(selected.phase);
                  setPickerVisible(true);
                }}
              >
                {t('productFactory.planning.retry')}
              </Button>
            )}
          </Space>
          {selected.state === 'candidate_ready' && selected.inputPlanRevision !== (run.planRevision ?? 1) && (
            <Alert className='mt-12px' type='warning' content={t('productFactory.planning.staleCandidate')} />
          )}
        </div>
      )}
      {!summary || summary.costUnknown ? (
        <p className='text-13px text-t-secondary'>{t('productFactory.planning.unknownCost')}</p>
      ) : null}
      {summary?.budgetLimitUsd != null && (
        <p className='text-13px text-t-secondary'>
          {t('productFactory.delivery.budgetSummary', {
            limit: summary.budgetLimitUsd.toFixed(2),
            remaining:
              summary.costUnknown || summary.budgetRemainingUsd == null
                ? t('productFactory.review.unknown')
                : summary.budgetRemainingUsd.toFixed(2),
          })}
        </p>
      )}
      {pickerVisible && (
        <Suspense fallback={<Spin />}>
          <TeamCreateModal
            visible
            onClose={() => setPickerVisible(false)}
            onCreated={() => undefined}
            creationOverride={{
              name: run.name,
              workspace: run.workspacePath,
              title: t('productFactory.planning.selectAssistant'),
              subtitle: t('productFactory.planning.oneAssistant'),
              confirmLabel: t('productFactory.planning.confirmGeneration'),
              submit: async (agents) => {
                if (agents.length !== 1 || !agents[0].assistant_id) {
                  setConfigurationError(true);
                  throw new Error(t('productFactory.planning.oneAssistant'));
                }
                const attempt = await planning.start(
                  phase,
                  run.planRevision ?? 1,
                  agents[0].assistant_id,
                  agents[0].model ?? ''
                );
                if (!attempt) throw new Error(t('productFactory.planning.requestError'));
                setSelectedId(attempt.id);
              },
            }}
          />
        </Suspense>
      )}
    </Card>
  );
}
