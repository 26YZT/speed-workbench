/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useEffect, useState } from 'react';
import { Alert, Button, Descriptions, Space, Tag } from '@arco-design/web-react';
import { ArrowLeft, Refresh } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import { checkProductFactoryWorkspace, getProductFactoryRunForPreview } from '../client';
import {
  asRecord,
  compareHandoffSnapshot,
  inspectHandoffDraft,
  isHandedOff,
  type HandoffIssue,
} from '../handoffPreview';
import type { ProductFactoryRun } from '../types';
import styles from '../ProductFactory.module.css';
import HandoffConfirm from './HandoffConfirm';

type Props = {
  run: ProductFactoryRun;
  onClose: () => void;
  onHandoff?: (run: ProductFactoryRun) => void;
  workflowBlocked?: boolean;
};

const TeamHandoffPreview: React.FC<Props> = ({ run, onClose, onHandoff, workflowBlocked = false }) => {
  const { t } = useTranslation();
  const [refresh, setRefresh] = useState(0);
  const [check, setCheck] = useState<{
    input: ProductFactoryRun;
    refresh: number;
    server?: ProductFactoryRun;
    issues: HandoffIssue[];
  }>();
  const checking = !check || check.input !== run || check.refresh !== refresh;
  useEffect(() => {
    let active = true;
    const inspect = async () => {
      try {
        const server = await getProductFactoryRunForPreview(run.id);
        const issues = compareHandoffSnapshot(run, server);
        const workspaceIssue = await checkProductFactoryWorkspace(server.workspacePath);
        if (workspaceIssue) issues.push({ code: workspaceIssue });
        if (active) setCheck({ input: run, refresh, server, issues });
      } catch {
        if (active)
          setCheck({
            input: run,
            refresh,
            issues: [...inspectHandoffDraft(run.taskDraft).issues, { code: 'serverUnavailable' }],
          });
      }
    };
    void inspect();
    return () => {
      active = false;
    };
  }, [run, refresh]);

  const snapshot = !checking && check?.server ? check.server : run;
  const draft = asRecord(snapshot.taskDraft);
  const { tasks } = inspectHandoffDraft(snapshot.taskDraft);
  const issues: HandoffIssue[] = [
    ...(checking ? [] : (check?.issues ?? [])),
    ...(workflowBlocked ? [{ code: 'versionNotReady' as const }] : []),
  ];
  const ready = !checking && issues.length === 0;
  const missing = t('productFactory.handoff.notProvided');
  const yesNo = (value: boolean) => (value ? t('productFactory.handoff.yes') : t('productFactory.handoff.no'));
  const typeLabels = {
    frontend: t('productFactory.handoff.types.frontend'),
    backend: t('productFactory.handoff.types.backend'),
    data: t('productFactory.handoff.types.data'),
    test: t('productFactory.handoff.types.test'),
  };
  const effortLabels = {
    low: t('productFactory.handoff.efforts.low'),
    medium: t('productFactory.handoff.efforts.medium'),
    high: t('productFactory.handoff.efforts.high'),
  };
  const statusLabels: Record<string, string> = {
    draft: t('productFactory.handoff.statuses.draft'),
    interviewing: t('productFactory.handoff.statuses.interviewing'),
    blueprint_generating: t('productFactory.handoff.statuses.blueprintGenerating'),
    blueprint_ready: t('productFactory.handoff.statuses.blueprintReady'),
    task_draft_generating: t('productFactory.handoff.statuses.taskDraftGenerating'),
    task_draft_ready: t('productFactory.handoff.statuses.taskDraftReady'),
    handed_off: t('productFactory.handoff.statuses.handedOff'),
    running: t('productFactory.handoff.statuses.running'),
    in_review: t('productFactory.handoff.statuses.inReview'),
    completed: t('productFactory.handoff.statuses.completed'),
    failed: t('productFactory.handoff.statuses.failed'),
  };
  return (
    <section aria-label={t('productFactory.handoff.title')} className={styles.handoffPreview}>
      <Space wrap>
        <Button icon={<ArrowLeft />} onClick={onClose}>
          {t('productFactory.handoff.close')}
        </Button>
        <Button icon={<Refresh />} loading={checking} onClick={() => setRefresh((value) => value + 1)}>
          {t('productFactory.handoff.refresh')}
        </Button>
      </Space>
      <h2>{t('productFactory.handoff.title')}</h2>
      <p className={styles.hint}>{t('productFactory.handoff.disclaimer')}</p>
      <div data-testid='handoff-result' aria-live='polite'>
        <Tag color={ready ? 'green' : 'orange'}>
          {ready ? t('productFactory.handoff.ready') : t('productFactory.handoff.blocked')}
        </Tag>
        {checking && <span>{t('productFactory.handoff.checking')}</span>}
      </div>
      {issues.length > 0 && (
        <Alert
          type='warning'
          className='mt-12px'
          content={
            <ul data-testid='handoff-reasons' className='m-0 pl-20px'>
              {issues.map((issue, index) => (
                <li key={index}>
                  {t(`productFactory.handoff.reasons.${issue.code}`, {
                    taskId: issue.taskId || missing,
                    dependency: issue.dependency || missing,
                  })}
                </li>
              ))}
            </ul>
          }
        />
      )}
      <Descriptions
        size='small'
        border
        column={2}
        className='mt-16px'
        data={[
          { label: t('productFactory.handoff.fields.productName'), value: snapshot.name },
          { label: t('productFactory.handoff.fields.status'), value: statusLabels[snapshot.status] ?? missing },
          { label: t('productFactory.handoff.fields.workspace'), value: snapshot.workspacePath || missing },
          {
            label: t('productFactory.handoff.fields.revision'),
            value: typeof draft.revision === 'number' ? draft.revision : missing,
          },
          { label: t('productFactory.handoff.fields.confirmed'), value: yesNo(draft.confirmed === true) },
          { label: t('productFactory.handoff.fields.taskCount'), value: tasks.length },
          { label: t('productFactory.handoff.fields.handedOff'), value: yesNo(isHandedOff(snapshot)) },
        ]}
      />
      {tasks.map((task, index) => (
        <Descriptions
          key={`${task.id}-${index}`}
          size='small'
          border
          column={2}
          className='mt-16px'
          data={[
            { label: t('productFactory.handoff.fields.id'), value: task.id || missing },
            { label: t('productFactory.handoff.fields.titleField'), value: task.title || missing },
            { label: t('productFactory.handoff.fields.description'), value: task.description || missing },
            { label: t('productFactory.handoff.fields.type'), value: typeLabels[task.type] ?? missing },
            {
              label: t('productFactory.handoff.fields.dependencies'),
              value: task.blockedBy.join(', ') || t('productFactory.handoff.none'),
            },
            {
              label: t('productFactory.handoff.fields.criteria'),
              value: task.acceptanceCriteria.length ? (
                <ul className='m-0 pl-20px'>
                  {task.acceptanceCriteria.map((item, i) => (
                    <li key={i}>{item || missing}</li>
                  ))}
                </ul>
              ) : (
                missing
              ),
            },
            { label: t('productFactory.handoff.fields.role'), value: task.suggestedRole || missing },
            { label: t('productFactory.handoff.fields.effort'), value: effortLabels[task.effort] ?? missing },
            ...(draft.version === 2
              ? [
                  {
                    label: t('productFactory.taskDraft.scopeField'),
                    value:
                      task.executionScope === 'project_integration'
                        ? t('productFactory.taskDraft.scopes.projectIntegration')
                        : t('productFactory.taskDraft.scopes.taskWorkspace'),
                  },
                  {
                    label: t('productFactory.taskDraft.requirementsField'),
                    value: task.requirementIds?.join(', ') || missing,
                  },
                ]
              : []),
          ]}
        />
      ))}
      {ready && onHandoff && <HandoffConfirm run={snapshot} onResult={onHandoff} />}
    </section>
  );
};

export default TeamHandoffPreview;
