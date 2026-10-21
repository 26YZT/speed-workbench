/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, Button, Modal } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import { usePageLeaveGuard } from '@renderer/hooks/context/NavigationHistoryContext';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  clearProductIdeaDraft,
  EMPTY_PRODUCT_IDEA_DRAFT,
  loadIdeaDraftRunId,
  saveIdeaDraftRunId,
  hasProductIdeaDraft,
  loadProductIdeaDraft,
  saveProductFactoryRunId,
  saveProductIdeaDraft,
} from './draftStore';
import {
  confirmProductFactoryBlueprint,
  confirmProductFactoryInterview,
  createProductFactoryRun,
  getProductFactoryRun,
  listProductFactoryRuns,
  saveProductFactoryBlueprint,
  saveProductFactoryInterview,
  confirmProductFactoryTaskDraft,
  generateProductFactoryTaskDraft,
  saveProductFactoryTaskDraft,
  continueProductFactoryRun,
  getProductFactoryReview,
  getProductFactoryExecution,
  getProductFactoryDelivery,
  reviewProductFactoryTask,
} from './client';
import type {
  BlueprintArtifact,
  InterviewArtifact,
  ProductFactoryRun,
  ProductIdeaDraft,
  TaskDraftArtifact,
} from './types';
import FactoryHome from './components/FactoryHome';
import IdeaDraftForm from './components/IdeaDraftForm';
import RunWorkspace from './components/RunWorkspace';
import styles from './ProductFactory.module.css';
import {
  artifactsEqual,
  editableArtifacts,
  readArtifactRecovery,
  writeArtifactRecovery,
  clearArtifactRecovery,
  resolveArtifactRecovery,
} from './artifactRecovery';
import type { EditableArtifacts, ArtifactRecovery } from './artifactRecovery';

const ProductFactoryPage: React.FC = () => {
  const { t } = useTranslation();
  const translate = useRef(t);
  translate.current = t;
  const location = useLocation();
  const navigate = useNavigate();
  const routeSegment = location.pathname.slice('/product-factory/'.length).split('/')[0];
  const routeRunId = routeSegment && routeSegment !== 'new' ? routeSegment : undefined;
  const [draft, setDraft] = useState(() => loadProductIdeaDraft());
  const [runs, setRuns] = useState<ProductFactoryRun[]>([]);
  const [loadingRuns, setLoadingRuns] = useState(true);
  const [apiUnavailable, setApiUnavailable] = useState(false);
  const [runId, setRunId] = useState<string>(() => loadIdeaDraftRunId());
  const [workspaceRun, setWorkspaceRun] = useState<ProductFactoryRun>();
  const [workspaceLoading, setWorkspaceLoading] = useState(false);
  const [workspaceError, setWorkspaceError] = useState<string>();
  const [savingArtifact, setSavingArtifact] = useState(false);
  const [artifactDirty, setArtifactDirty] = useState(false);
  const [review, setReview] = useState<import('./types').ProductFactoryReview>();
  const [reviewLoading, setReviewLoading] = useState(false);
  const [reviewError, setReviewError] = useState<string>();
  const [reviewBusy, setReviewBusy] = useState(false);
  const [delivery, setDelivery] = useState<import('./types').ProductFactoryDelivery>();
  const showIdeaForm = location.pathname.endsWith('/new');
  const showWorkspace = Boolean(routeRunId) && !showIdeaForm;
  const routeRef = useRef(routeRunId);
  routeRef.current = routeRunId;
  const currentRun = useRef(workspaceRun);
  currentRun.current = workspaceRun;
  const baseline = useRef<EditableArtifacts>({});
  const loadSequence = useRef(0);
  const ideaEpoch = useRef(0);
  const reviewSequence = useRef(0);
  const deliverySequence = useRef(0);
  const reviewAction = useRef<object | undefined>(undefined);
  const saveAction = useRef<object | undefined>(undefined);
  useEffect(() => {
    reviewSequence.current++;
    deliverySequence.current++;
    reviewAction.current = undefined;
    saveAction.current = undefined;
    setSavingArtifact(false);
    setReviewBusy(false);
    setReviewLoading(false);
    setReview(undefined);
    setReviewError(undefined);
    setDelivery(undefined);
    return () => {
      reviewSequence.current++;
      deliverySequence.current++;
    };
  }, [routeRunId]);
  useEffect(
    () => () => {
      ideaEpoch.current++;
    },
    []
  );
  const [recoveryConflict, setRecoveryConflict] = useState<ArtifactRecovery>();
  const [restoredEdits, setRestoredEdits] = useState(false);
  const [localSaveError, setLocalSaveError] = useState(false);
  usePageLeaveGuard(
    artifactDirty || localSaveError,
    {
      title: t('common.draftProtection.leaveTitle'),
      content: t(localSaveError ? 'productFactory.recovery.saveError' : 'productFactory.recovery.leave'),
      leave: t(localSaveError ? 'common.draftProtection.leaveUnsafe' : 'common.draftProtection.leave'),
      stay: t('common.draftProtection.stay'),
    },
    localSaveError
  );
  useEffect(() => {
    if (!routeRunId) {
      setArtifactDirty(false);
      setRecoveryConflict(undefined);
      setRestoredEdits(false);
    }
  }, [routeRunId]);

  const loadRuns = useCallback(async () => {
    setLoadingRuns(true);
    try {
      const remoteRuns = await listProductFactoryRuns();
      setRuns(remoteRuns);
      setApiUnavailable(false);
    } catch {
      setApiUnavailable(true);
    } finally {
      setLoadingRuns(false);
    }
  }, []);

  useEffect(() => {
    if (!showIdeaForm && !showWorkspace) void loadRuns();
  }, [loadRuns, showIdeaForm, showWorkspace]);

  const loadWorkspace = useCallback(async () => {
    if (!routeRunId) return;
    const sequence = ++loadSequence.current;
    setRecoveryConflict(undefined);
    setRestoredEdits(false);
    setArtifactDirty(false);
    setLocalSaveError(false);
    currentRun.current = undefined;
    setWorkspaceRun(undefined);
    setWorkspaceLoading(true);
    setWorkspaceError(undefined);
    try {
      const loaded = await getProductFactoryRun(routeRunId);
      if (sequence !== loadSequence.current || routeRef.current !== routeRunId) return;
      baseline.current = editableArtifacts(loaded);
      setRecoveryConflict(undefined);
      setRestoredEdits(false);
      const recovery = readArtifactRecovery(loaded.id);
      const resolution = recovery ? resolveArtifactRecovery(loaded, recovery) : undefined;
      const next = resolution === 'restore' && recovery ? { ...loaded, ...recovery.edited } : loaded;
      if (resolution === 'saved') clearArtifactRecovery(loaded.id);
      if (resolution === 'conflict') setRecoveryConflict(recovery);
      currentRun.current = next;
      setWorkspaceRun(next);
      setArtifactDirty(resolution === 'restore');
      setRestoredEdits(resolution === 'restore');
      saveProductFactoryRunId(loaded.id);
    } catch (caught) {
      if (sequence === loadSequence.current && routeRef.current === routeRunId)
        setWorkspaceError(caught instanceof Error ? caught.message : 'PRODUCT_FACTORY_LOAD_ERROR');
    } finally {
      if (sequence === loadSequence.current) setWorkspaceLoading(false);
    }
  }, [routeRunId]);

  useEffect(() => {
    if (showWorkspace) void loadWorkspace();
  }, [loadWorkspace, showWorkspace]);

  const loadReview = useCallback(
    async (silent = false) => {
      if (!routeRunId || !workspaceRun?.teamId) return;
      const id = routeRunId;
      const sequence = ++reviewSequence.current;
      const isCurrent = () => routeRef.current === id && sequence === reviewSequence.current;
      if (!silent) setReviewLoading(true);
      try {
        const execution = await getProductFactoryExecution(routeRunId);
        if (!isCurrent()) return;
        if (!execution || execution.state === 'pending') {
          setReview(undefined);
          return;
        }
        const snapshot = await getProductFactoryReview(routeRunId);
        if (!isCurrent()) return;
        setReviewError(undefined);
        setReview(snapshot);
        setWorkspaceRun((current) => {
          if (current?.id !== snapshot.run.id) return current;
          currentRun.current = snapshot.run;
          return snapshot.run;
        });
      } catch {
        if (isCurrent()) setReviewError(translate.current('productFactory.review.loadError'));
      } finally {
        if (isCurrent()) setReviewLoading(false);
      }
    },
    [routeRunId, workspaceRun?.teamId]
  );

  const loadDelivery = useCallback(async () => {
    const sequence = ++deliverySequence.current;
    if (!routeRunId || !workspaceRun?.teamId || workspaceRun.status !== 'completed') {
      setDelivery(undefined);
      return;
    }
    try {
      const result = await getProductFactoryDelivery(routeRunId);
      if (routeRef.current === routeRunId && sequence === deliverySequence.current) setDelivery(result);
    } catch {
      if (routeRef.current === routeRunId && sequence === deliverySequence.current) setDelivery(undefined);
    }
  }, [routeRunId, workspaceRun?.status, workspaceRun?.teamId]);

  useEffect(() => {
    if (workspaceRun?.teamId) void loadReview();
    else setReview(undefined);
  }, [loadReview, workspaceRun?.teamId]);

  useEffect(() => {
    void loadDelivery();
  }, [loadDelivery]);

  useEffect(() => {
    if (!workspaceRun?.teamId || ['completed', 'failed'].includes(workspaceRun.status) || reviewBusy) return;
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void loadReview(true);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [loadReview, reviewBusy, workspaceRun?.status, workspaceRun?.teamId]);

  const applyReview = async (decision: 'approve' | 'request_changes', feedback?: string) => {
    if (!routeRunId || reviewAction.current) return;
    const id = routeRunId;
    const action = {};
    reviewAction.current = action;
    reviewSequence.current++;
    setReviewBusy(true);
    setReviewError(undefined);
    try {
      const result = await reviewProductFactoryTask(id, decision, feedback);
      if (routeRef.current !== id || reviewAction.current !== action) return;
      setReview(result);
      await loadWorkspace();
    } catch {
      if (routeRef.current === id && reviewAction.current === action)
        setReviewError(t('productFactory.workspace.actionError'));
    } finally {
      if (routeRef.current === id && reviewAction.current === action) {
        reviewAction.current = undefined;
        setReviewBusy(false);
      }
    }
  };

  const continueReview = async () => {
    if (!routeRunId || reviewAction.current) return;
    const id = routeRunId;
    const action = {};
    reviewAction.current = action;
    reviewSequence.current++;
    setReviewBusy(true);
    setReviewError(undefined);
    try {
      await continueProductFactoryRun(routeRunId);
      if (routeRef.current !== id || reviewAction.current !== action) return;
      await loadWorkspace();
      await loadReview();
    } catch {
      if (routeRef.current === id && reviewAction.current === action)
        setReviewError(t('productFactory.workspace.actionError'));
    } finally {
      if (routeRef.current === id && reviewAction.current === action) {
        reviewAction.current = undefined;
        setReviewBusy(false);
      }
    }
  };

  const changeArtifact = (field: keyof EditableArtifacts, artifact: EditableArtifacts[keyof EditableArtifacts]) => {
    if (recoveryConflict) return;
    const current = currentRun.current;
    if (!current || current.id !== routeRef.current) return;
    const next = { ...current, [field]: artifact };
    currentRun.current = next;
    setWorkspaceRun(next);
    const dirty = !artifactsEqual(editableArtifacts(next), baseline.current);
    setArtifactDirty(dirty);
    if (dirty) setLocalSaveError(!writeArtifactRecovery(current.id, baseline.current, editableArtifacts(next)));
    else {
      clearArtifactRecovery(current.id);
      setLocalSaveError(false);
    }
  };

  const acceptSavedRun = (updated: ProductFactoryRun, submitted: EditableArtifacts, field: keyof EditableArtifacts) => {
    if (routeRef.current !== updated.id || currentRun.current?.id !== updated.id) {
      const recovery = readArtifactRecovery(updated.id);
      if (recovery && resolveArtifactRecovery(updated, recovery) === 'saved') clearArtifactRecovery(updated.id);
      return;
    }
    const current = editableArtifacts(currentRun.current);
    const next = { ...updated };
    for (const key of ['interview', 'blueprint', 'taskDraft'] as const) {
      const before = key === field ? submitted[key] : baseline.current[key];
      if (!artifactsEqual({ ...baseline.current, [key]: current[key] }, { ...baseline.current, [key]: before })) {
        Object.assign(next, { [key]: current[key] });
      }
    }
    baseline.current = editableArtifacts(updated);
    currentRun.current = next;
    setWorkspaceRun(next);
    const dirty = !artifactsEqual(editableArtifacts(next), baseline.current);
    setArtifactDirty(dirty);
    setRestoredEdits(false);
    if (dirty) setLocalSaveError(!writeArtifactRecovery(updated.id, baseline.current, editableArtifacts(next)));
    else {
      clearArtifactRecovery(updated.id);
      setLocalSaveError(false);
    }
  };

  const runSave = async (
    field: keyof EditableArtifacts,
    save: () => Promise<ProductFactoryRun>,
    confirm?: (saved: ProductFactoryRun) => Promise<ProductFactoryRun>
  ) => {
    if (!routeRunId || saveAction.current) return;
    const id = routeRunId;
    const action = {};
    saveAction.current = action;
    const submitted = currentRun.current ? editableArtifacts(currentRun.current) : {};
    setSavingArtifact(true);
    try {
      const saved = await save();
      acceptSavedRun(saved, submitted, field);
      if (confirm) acceptSavedRun(await confirm(saved), editableArtifacts(saved), field);
    } finally {
      if (saveAction.current === action) {
        saveAction.current = undefined;
        if (routeRef.current === id) setSavingArtifact(false);
      }
    }
  };
  const saveInterview = (artifact: InterviewArtifact) =>
    runSave('interview', () => saveProductFactoryInterview(routeRunId!, artifact));
  const confirmInterview = (artifact: InterviewArtifact) =>
    runSave(
      'interview',
      () => saveProductFactoryInterview(routeRunId!, { ...artifact, confirmed: true }),
      () => confirmProductFactoryInterview(routeRunId!)
    );
  const saveBlueprint = (artifact: BlueprintArtifact) =>
    runSave('blueprint', () => saveProductFactoryBlueprint(routeRunId!, artifact));
  const confirmBlueprint = (artifact: BlueprintArtifact) =>
    runSave(
      'blueprint',
      () => saveProductFactoryBlueprint(routeRunId!, { ...artifact, confirmed: true }),
      () => confirmProductFactoryBlueprint(routeRunId!)
    );
  const generateTaskDraft = () => runSave('taskDraft', () => generateProductFactoryTaskDraft(routeRunId!));
  const saveTaskDraft = (artifact: TaskDraftArtifact) =>
    runSave('taskDraft', () => saveProductFactoryTaskDraft(routeRunId!, artifact));
  const confirmTaskDraft = (artifact: TaskDraftArtifact) =>
    runSave(
      'taskDraft',
      () => saveProductFactoryTaskDraft(routeRunId!, artifact),
      (saved) => confirmProductFactoryTaskDraft(routeRunId!, (saved.taskDraft as TaskDraftArtifact).revision)
    );

  const keepIdeaDraft = (next: ProductIdeaDraft) => {
    setDraft(next);
    try {
      saveProductIdeaDraft(next, undefined, true);
      setLocalSaveError(false);
    } catch {
      setLocalSaveError(true);
    }
  };
  const exportEdits = () => {
    const recovery =
      recoveryConflict ??
      (showWorkspace && currentRun.current
        ? {
            version: 1,
            runId: currentRun.current.id,
            baseline: baseline.current,
            edited: editableArtifacts(currentRun.current),
          }
        : draft);
    const url = URL.createObjectURL(new Blob([JSON.stringify(recovery, null, 2)], { type: 'application/json' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'product-draft.json';
    link.click();
    URL.revokeObjectURL(url);
  };
  const startNewIdea = () => {
    const reset = () => {
      ideaEpoch.current++;
      try {
        clearProductIdeaDraft();
      } catch {
        setLocalSaveError(true);
        return;
      }
      setDraft({ ...EMPTY_PRODUCT_IDEA_DRAFT });
      setRunId('');
      setLocalSaveError(false);
      void navigate('/product-factory/new');
    };
    if (hasProductIdeaDraft(draft)) {
      Modal.confirm({
        title: t('productFactory.home.replaceTitle'),
        content: t('productFactory.home.replaceDescription'),
        okText: t('productFactory.home.newIdea'),
        cancelText: t('common.cancel'),
        onOk: reset,
      });
    } else reset();
  };

  const persistDraft = async (nextDraft: ProductIdeaDraft): Promise<ProductIdeaDraft> => {
    const epoch = ideaEpoch.current;
    const normalized = saveProductIdeaDraft(nextDraft);
    setDraft(normalized);
    if (!runId || apiUnavailable) {
      try {
        const run = await createProductFactoryRun(normalized);
        if (epoch !== ideaEpoch.current) return normalized;
        setRunId(run.id);
        saveIdeaDraftRunId(run.id);
        saveProductFactoryRunId(run.id);
        setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)]);
        setApiUnavailable(false);
      } catch {
        // The draft is saved locally even when optional server sync fails.
        setApiUnavailable(true);
      }
    }
    return normalized;
  };

  return (
    <main className={styles.page} data-testid='product-factory-page'>
      <div className={styles.content}>
        {localSaveError && (
          <Alert
            className='mb-16px'
            type='error'
            content={t('productFactory.recovery.saveError')}
            action={<Button onClick={exportEdits}>{t('common.draftProtection.exportDraft')}</Button>}
          />
        )}
        {showWorkspace && artifactDirty && (
          <div
            className='mb-16px rounded-8px border border-border-2 bg-fill-1 p-12px text-13px text-t-secondary'
            role='status'
          >
            {t(restoredEdits ? 'productFactory.recovery.restored' : 'productFactory.recovery.local')}
          </div>
        )}
        {showWorkspace && recoveryConflict && (
          <Alert
            className='mb-16px'
            type='warning'
            content={t('productFactory.recovery.conflict', { name: workspaceRun?.name ?? '' })}
            action={
              <div className='flex gap-8px'>
                <Button onClick={exportEdits}>{t('common.draftProtection.exportDraft')}</Button>
                <Button
                  onClick={() =>
                    Modal.confirm({
                      title: t('productFactory.recovery.discardTitle'),
                      content: t('productFactory.recovery.discardDescription'),
                      okText: t('common.confirm'),
                      cancelText: t('common.cancel'),
                      onOk: () => {
                        if (routeRunId) clearArtifactRecovery(routeRunId);
                        setRecoveryConflict(undefined);
                      },
                    })
                  }
                >
                  {t('productFactory.recovery.discard')}
                </Button>
              </div>
            }
          />
        )}
        {showIdeaForm ? (
          <IdeaDraftForm
            initialDraft={draft}
            onDraftChange={keepIdeaDraft}
            localSaveError={localSaveError}
            onBack={() => void navigate('/product-factory')}
            onSave={persistDraft}
            onStartAnalysis={async (nextDraft) => {
              const epoch = ideaEpoch.current;
              const normalized = saveProductIdeaDraft(nextDraft);
              setDraft(normalized);
              // Starting a new product always creates a new run, regardless of
              // whether another run is selected in the home page or storage.
              const run = await createProductFactoryRun(normalized);
              if (epoch !== ideaEpoch.current) return;
              setRunId(run.id);
              saveProductFactoryRunId(run.id);
              setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)]);
              setApiUnavailable(false);
              try {
                clearProductIdeaDraft();
              } catch {
                // The server already committed this run. A stale local draft
                // must not hide it or make creation look unsuccessful.
              }
              setDraft({ ...EMPTY_PRODUCT_IDEA_DRAFT });
              setRunId('');
              void navigate(`/product-factory/${encodeURIComponent(run.id)}`);
            }}
          />
        ) : showWorkspace ? (
          <div inert={Boolean(recoveryConflict)}>
            <RunWorkspace
              run={workspaceRun}
              loading={workspaceLoading}
              error={workspaceError}
              onBack={() => void navigate('/product-factory')}
              onRetry={() => void loadWorkspace()}
              onInterviewChange={(artifact) => changeArtifact('interview', artifact)}
              onInterviewSave={saveInterview}
              onInterviewConfirm={confirmInterview}
              onBlueprintChange={(artifact) => changeArtifact('blueprint', artifact)}
              onBlueprintSave={saveBlueprint}
              onBlueprintConfirm={confirmBlueprint}
              onTaskDraftChange={(artifact) => changeArtifact('taskDraft', artifact)}
              hasUnsavedChanges={artifactDirty}
              onPlanningApplied={(updated) => {
                if (updated.id === routeRunId) {
                  baseline.current = editableArtifacts(updated);
                  currentRun.current = updated;
                  setWorkspaceRun(updated);
                  setArtifactDirty(false);
                  const recovery = readArtifactRecovery(updated.id);
                  if (recovery && resolveArtifactRecovery(updated, recovery) !== 'saved') setRecoveryConflict(recovery);
                  else clearArtifactRecovery(updated.id);
                }
              }}
              onTaskDraftGenerate={generateTaskDraft}
              onTaskDraftSave={saveTaskDraft}
              onTaskDraftConfirm={confirmTaskDraft}
              saving={savingArtifact}
              onHandoff={(updated) => {
                if (updated.id === routeRunId) setWorkspaceRun(updated);
              }}
              onOpenTeam={(teamId) => void navigate(`/team/${encodeURIComponent(teamId)}`)}
              onOpenVersionRun={(versionRunId) => void navigate(`/product-factory/${encodeURIComponent(versionRunId)}`)}
              review={review}
              reviewLoading={reviewLoading}
              reviewError={reviewError}
              reviewBusy={reviewBusy}
              onReviewReload={() => {
                void loadReview();
                void loadDelivery();
              }}
              onApproveReview={(feedback) => applyReview('approve', feedback)}
              onRequestChanges={(feedback) => applyReview('request_changes', feedback)}
              onContinueReview={continueReview}
              delivery={delivery}
            />
          </div>
        ) : (
          <FactoryHome
            draft={draft}
            runs={runs}
            loadingRuns={loadingRuns}
            apiUnavailable={apiUnavailable}
            onRetry={() => void loadRuns()}
            onCreate={startNewIdea}
            onContinue={() => void navigate('/product-factory/new')}
            onOpenRun={(id) => void navigate(`/product-factory/${encodeURIComponent(id)}`)}
          />
        )}
      </div>
    </main>
  );
};

export default ProductFactoryPage;
