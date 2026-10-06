/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useMemo, useState } from 'react';
import { Alert, Button, Spin, Tag } from '@arco-design/web-react';
import { ArrowLeft } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type {
  BlueprintArtifact,
  InterviewArtifact,
  ProductFactoryDelivery,
  ProductFactoryReview,
  ProductFactoryRun,
  TaskDraftArtifact,
} from '../types';
import BlueprintReview from './Preparation/BlueprintReview';
import InterviewPanel from './Preparation/InterviewPanel';
import TaskDraftReview from './Preparation/TaskDraftReview';
import TeamHandoffPreview from './TeamHandoffPreview';
import ExecutionStart from './ExecutionStart';
import ReviewPanel from './ReviewPanel';
import DeliveryPanel from './Delivery';
import styles from '../ProductFactory.module.css';
import { asRecord } from '../handoffPreview';
import PlanningPanel from './Preparation/Planning';
import VersionPanel from './Delivery/VersionPanel';

const toStringArray = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];

type RunWorkspaceProps = {
  run: ProductFactoryRun | undefined;
  loading: boolean;
  error?: string;
  onBack: () => void;
  onRetry: () => void;
  onInterviewChange: (artifact: InterviewArtifact) => void;
  onInterviewSave: (artifact: InterviewArtifact) => Promise<void>;
  onInterviewConfirm: (artifact: InterviewArtifact) => Promise<void>;
  onBlueprintChange: (artifact: BlueprintArtifact) => void;
  onBlueprintSave: (artifact: BlueprintArtifact) => Promise<void>;
  onBlueprintConfirm: (artifact: BlueprintArtifact) => Promise<void>;
  onTaskDraftChange: (artifact: TaskDraftArtifact) => void;
  onTaskDraftGenerate: () => Promise<void>;
  onTaskDraftSave: (artifact: TaskDraftArtifact) => Promise<void>;
  onTaskDraftConfirm: (artifact: TaskDraftArtifact) => Promise<void>;
  saving: boolean;
  onHandoff?: (run: ProductFactoryRun) => void;
  onOpenTeam?: (teamId: string) => void;
  review?: ProductFactoryReview;
  reviewLoading?: boolean;
  reviewError?: string;
  reviewBusy?: boolean;
  onReviewReload?: () => void;
  onApproveReview?: () => Promise<void>;
  onRequestChanges?: (feedback: string) => Promise<void>;
  onContinueReview?: () => Promise<void>;
  delivery?: ProductFactoryDelivery;
  onPlanningApplied?: (run: ProductFactoryRun) => void;
  hasUnsavedChanges?: boolean;
  onOpenVersionRun?: (runId: string) => void;
};

const defaultInterview = (
  run: ProductFactoryRun,
  translate: (key: string, options?: Record<string, string>) => string
): InterviewArtifact => ({
  version: 1,
  questions: [
    {
      id: 'audience',
      prompt: translate('productFactory.interview.questions.audience', { name: run.name }),
      answer: run.targetUser,
      notSure: false,
    },
    {
      id: 'problem',
      prompt: translate('productFactory.interview.questions.problem'),
      answer: run.problem,
      notSure: false,
    },
    {
      id: 'output',
      prompt: translate('productFactory.interview.questions.output'),
      answer: run.expectedOutput,
      notSure: false,
    },
  ],
  summary: '',
  confirmed: false,
  updatedAt: Date.now(),
});

const normalizeInterview = (
  run: ProductFactoryRun,
  translate: (key: string, options?: Record<string, string>) => string
): InterviewArtifact => {
  if (run.interview && typeof run.interview === 'object') {
    const raw = asRecord(run.interview);
    return {
      version: raw.version === 2 ? 2 : 1,
      questions: Array.isArray(raw.questions)
        ? raw.questions.map((value) => {
            const question = asRecord(value);
            return {
              id: typeof question.id === 'string' ? question.id : '',
              prompt:
                typeof question.prompt === 'string'
                  ? question.prompt
                  : typeof question.question === 'string'
                    ? question.question
                    : '',
              answer: typeof question.answer === 'string' ? question.answer : '',
              notSure: question.notSure === true || question.not_sure === true,
              reason: typeof question.reason === 'string' ? question.reason : undefined,
            };
          })
        : [],
      summary: typeof raw.summary === 'string' ? raw.summary : '',
      confirmed: raw.confirmed === true,
      updatedAt:
        typeof raw.updatedAt === 'number'
          ? raw.updatedAt
          : typeof raw.updated_at === 'number'
            ? raw.updated_at
            : Date.now(),
      generatedBy: raw.generatedBy === 'model' || raw.generated_by === 'model' ? 'model' : 'draft',
    };
  }
  return defaultInterview(run, translate);
};

const normalizeBlueprint = (run: ProductFactoryRun): BlueprintArtifact => {
  if (run.blueprint && typeof run.blueprint === 'object') {
    const raw = run.blueprint as Partial<BlueprintArtifact> & {
      open_questions?: unknown;
      generated_by?: unknown;
      updated_at?: unknown;
    };
    return {
      version: raw.version === 2 ? 2 : 1,
      sections: Array.isArray(raw.sections) ? raw.sections : [],
      risks: toStringArray(raw.risks),
      openQuestions: toStringArray(raw.openQuestions ?? raw.open_questions),
      generatedBy: raw.generatedBy === 'model' || raw.generated_by === 'model' ? 'model' : 'draft',
      confirmed: raw.confirmed === true,
      requirements: Array.isArray(raw.requirements)
        ? raw.requirements.map((value) => {
            const requirement = asRecord(value);
            return {
              id: typeof requirement.id === 'string' ? requirement.id : '',
              title: typeof requirement.title === 'string' ? requirement.title : '',
              acceptanceCriteria: toStringArray(requirement.acceptanceCriteria ?? requirement.acceptance_criteria),
            };
          })
        : [],
      updatedAt:
        typeof raw.updatedAt === 'number'
          ? raw.updatedAt
          : typeof raw.updated_at === 'number'
            ? raw.updated_at
            : Date.now(),
    };
  }
  return {
    version: 1,
    sections: [],
    risks: [],
    openQuestions: [],
    generatedBy: 'draft',
    confirmed: false,
    updatedAt: Date.now(),
  };
};

const RunWorkspace: React.FC<RunWorkspaceProps> = ({
  run,
  loading,
  error,
  onBack,
  onRetry,
  onInterviewChange,
  onInterviewSave,
  onInterviewConfirm,
  onBlueprintChange,
  onBlueprintSave,
  onBlueprintConfirm,
  onTaskDraftChange,
  onTaskDraftGenerate,
  onTaskDraftSave,
  onTaskDraftConfirm,
  saving,
  onHandoff,
  onOpenTeam,
  review,
  reviewLoading = false,
  reviewError,
  reviewBusy = false,
  onReviewReload,
  onApproveReview,
  onRequestChanges,
  onContinueReview,
  delivery,
  onPlanningApplied,
  hasUnsavedChanges = false,
  onOpenVersionRun,
}) => {
  const { t } = useTranslation();
  const interview = useMemo(() => (run ? normalizeInterview(run, t) : undefined), [run, t]);
  const blueprint = useMemo(() => (run ? normalizeBlueprint(run) : undefined), [run]);
  const [actionError, setActionError] = useState<string>();
  const [previewRunId, setPreviewRunId] = useState<string>();
  const [planningActive, setPlanningActive] = useState(false);
  const [versionBlocked, setVersionBlocked] = useState(false);
  const preparationBusy = saving || planningActive || versionBlocked;
  const execute = async (action: () => Promise<void>) => {
    setActionError(undefined);
    try {
      await action();
    } catch (caught) {
      setActionError(caught instanceof Error ? caught.message : t('productFactory.workspace.actionError'));
    }
  };

  if (loading)
    return (
      <div className={styles.workspaceState}>
        <Spin /> <span>{t('productFactory.workspace.loading')}</span>
      </div>
    );
  if (error || !run) {
    return (
      <div className={styles.workspaceState} role='alert'>
        <p>{error ?? t('productFactory.workspace.notFound')}</p>
        <Button onClick={onRetry}>{t('productFactory.workspace.retry')}</Button>
      </div>
    );
  }

  const showTaskDraft = run.status === 'task_draft_generating' || run.status === 'task_draft_ready';
  const showBlueprint = run.status === 'blueprint_ready';
  const taskDraft = run.taskDraft as TaskDraftArtifact | undefined;

  return (
    <div className='flex flex-col gap-18px'>
      <Button type='text' icon={<ArrowLeft size={16} />} onClick={onBack}>
        {t('productFactory.form.back')}
      </Button>
      <header className='flex items-start justify-between gap-16px max-720px:flex-col'>
        <div>
          <div className='mb-8px text-12px font-600 uppercase tracking-1px text-brand-6'>
            {t('productFactory.home.eyebrow')}
          </div>
          <h1 className='m-0 text-26px font-650 leading-36px text-t-primary'>{run.name}</h1>
          <p className='m-0 mt-8px max-w-720px text-14px leading-22px text-t-secondary'>{run.idea}</p>
        </div>
        <Tag color='arcoblue'>{run.status}</Tag>
      </header>
      {actionError && (
        <div
          className='rounded-8px border border-warning-3 bg-warning-1 px-14px py-10px text-13px text-warning-7'
          role='alert'
        >
          {actionError}
        </div>
      )}
      {onOpenVersionRun && (
        <VersionPanel key={run.id} run={run} onOpenRun={onOpenVersionRun} onPreparationBlocked={setVersionBlocked} />
      )}
      {run.teamId ? (
        <section>
          <Alert type='success' content={t('productFactory.handoff.execution.success')} />
          <p>
            {t('productFactory.handoff.execution.teamId')}: {run.teamId}
          </p>
          <p>{t('productFactory.handoff.execution.openTeamWarning')}</p>
          <Button onClick={() => onOpenTeam?.(run.teamId!)}>{t('productFactory.handoff.execution.openTeam')}</Button>
          {review || reviewLoading || reviewError ? (
            <>
              {run.status === 'completed' &&
                (delivery ? (
                  <DeliveryPanel delivery={delivery} />
                ) : (
                  <Alert type='warning' content={t('productFactory.delivery.loadError')} />
                ))}
              <ReviewPanel
                review={review}
                loading={reviewLoading}
                busy={reviewBusy}
                error={reviewError}
                onReload={onReviewReload ?? (() => undefined)}
                onApprove={onApproveReview ?? (async () => undefined)}
                onRequestChanges={onRequestChanges ?? (async () => undefined)}
                onContinue={onContinueReview ?? (async () => undefined)}
              />
            </>
          ) : (
            <ExecutionStart key={run.id} run={run} />
          )}
        </section>
      ) : previewRunId === run.id ? (
        <TeamHandoffPreview
          run={run}
          workflowBlocked={versionBlocked}
          onClose={() => setPreviewRunId(undefined)}
          onHandoff={onHandoff}
        />
      ) : (
        <>
          {onPlanningApplied && (
            <PlanningPanel
              key={run.id}
              run={run}
              disabled={saving || hasUnsavedChanges || versionBlocked}
              onApplied={onPlanningApplied}
              onActiveChange={setPlanningActive}
            />
          )}
          <p className='m-0 text-13px text-t-secondary'>{t('productFactory.planning.rulesAvailable')}</p>
          <Button onClick={() => setPreviewRunId(run.id)} disabled={preparationBusy}>
            {t('productFactory.handoff.open')}
          </Button>
          {showTaskDraft && taskDraft ? (
            <TaskDraftReview
              artifact={taskDraft}
              onChange={onTaskDraftChange}
              onGenerate={() => void execute(onTaskDraftGenerate)}
              onSave={() => void execute(() => onTaskDraftSave(taskDraft))}
              onConfirm={() => void execute(() => onTaskDraftConfirm(taskDraft))}
              saving={preparationBusy}
            />
          ) : showTaskDraft ? (
            <div className={styles.workspaceState}>
              <Spin /> <span>{t('productFactory.taskDraft.generating')}</span>
            </div>
          ) : showBlueprint && blueprint ? (
            <>
              <BlueprintReview
                artifact={blueprint}
                onChange={onBlueprintChange}
                onSave={() => void execute(() => onBlueprintSave(blueprint))}
                onConfirm={() => void execute(() => onBlueprintConfirm(blueprint))}
                saving={preparationBusy}
              />
              {blueprint.confirmed && (
                <Button
                  type='primary'
                  loading={saving}
                  disabled={preparationBusy}
                  onClick={() => void execute(onTaskDraftGenerate)}
                >
                  {t('productFactory.taskDraft.generate')}
                </Button>
              )}
            </>
          ) : (
            interview && (
              <InterviewPanel
                artifact={interview}
                onChange={onInterviewChange}
                onSave={() => void execute(() => onInterviewSave(interview))}
                onConfirm={() => void execute(() => onInterviewConfirm(interview))}
                saving={preparationBusy}
              />
            )
          )}
        </>
      )}
    </div>
  );
};

export default RunWorkspace;
