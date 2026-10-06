/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useCallback, useEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  loadProductFactoryRunId,
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

const ProductFactoryPage: React.FC = () => {
  const location = useLocation();
  const navigate = useNavigate();
  const routeSegment = location.pathname.slice('/product-factory/'.length).split('/')[0];
  const routeRunId = routeSegment && routeSegment !== 'new' ? routeSegment : undefined;
  const [draft, setDraft] = useState(() => loadProductIdeaDraft());
  const [runs, setRuns] = useState<ProductFactoryRun[]>([]);
  const [loadingRuns, setLoadingRuns] = useState(true);
  const [apiUnavailable, setApiUnavailable] = useState(false);
  const [runId, setRunId] = useState<string>(() => loadProductFactoryRunId() ?? '');
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

  const loadRuns = useCallback(async () => {
    setLoadingRuns(true);
    try {
      const remoteRuns = await listProductFactoryRuns();
      setRuns(remoteRuns);
      setApiUnavailable(false);
      const latest = remoteRuns[0];
      if (latest) {
        setRunId(latest.id);
        saveProductFactoryRunId(latest.id);
        setDraft(saveProductIdeaDraft(latest));
      }
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
    setWorkspaceLoading(true);
    setWorkspaceError(undefined);
    try {
      const loaded = await getProductFactoryRun(routeRunId);
      setWorkspaceRun(loaded);
      setArtifactDirty(false);
      setRunId(loaded.id);
      saveProductFactoryRunId(loaded.id);
      setDraft(saveProductIdeaDraft(loaded));
    } catch (caught) {
      setWorkspaceError(caught instanceof Error ? caught.message : 'PRODUCT_FACTORY_LOAD_ERROR');
    } finally {
      setWorkspaceLoading(false);
    }
  }, [routeRunId]);

  useEffect(() => {
    if (showWorkspace) void loadWorkspace();
  }, [loadWorkspace, showWorkspace]);

  const loadReview = useCallback(
    async (silent = false) => {
      if (!routeRunId || !workspaceRun?.teamId) return;
      if (!silent) setReviewLoading(true);
      setReviewError(undefined);
      try {
        const execution = await getProductFactoryExecution(routeRunId);
        if (!execution || execution.state !== 'enqueued') {
          setReview(undefined);
          return;
        }
        const snapshot = await getProductFactoryReview(routeRunId);
        setReview(snapshot);
        setWorkspaceRun((current) => (current?.id === snapshot.run.id ? snapshot.run : current));
      } catch (caught) {
        setReview(undefined);
        setReviewError(caught instanceof Error ? caught.message : 'PRODUCT_FACTORY_REVIEW_LOAD_ERROR');
      } finally {
        setReviewLoading(false);
      }
    },
    [routeRunId, workspaceRun?.teamId]
  );

  const loadDelivery = useCallback(async () => {
    if (!routeRunId || !workspaceRun?.teamId || workspaceRun.status !== 'completed') {
      setDelivery(undefined);
      return;
    }
    try {
      setDelivery(await getProductFactoryDelivery(routeRunId));
    } catch {
      setDelivery(undefined);
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
    if (!workspaceRun?.teamId || workspaceRun.status === 'completed' || reviewBusy) return;
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void loadReview(true);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [loadReview, reviewBusy, workspaceRun?.status, workspaceRun?.teamId]);

  const applyReview = async (decision: 'approve' | 'request_changes', feedback?: string) => {
    if (!routeRunId) return;
    setReviewBusy(true);
    setReviewError(undefined);
    try {
      setReview(await reviewProductFactoryTask(routeRunId, decision, feedback));
      await loadWorkspace();
    } catch (caught) {
      setReviewError(caught instanceof Error ? caught.message : 'PRODUCT_FACTORY_REVIEW_ERROR');
    } finally {
      setReviewBusy(false);
    }
  };

  const continueReview = async () => {
    if (!routeRunId) return;
    setReviewBusy(true);
    setReviewError(undefined);
    try {
      await continueProductFactoryRun(routeRunId);
      await loadWorkspace();
      await loadReview();
    } catch (caught) {
      setReviewError(caught instanceof Error ? caught.message : 'PRODUCT_FACTORY_CONTINUE_ERROR');
    } finally {
      setReviewBusy(false);
    }
  };

  const saveInterview = async (artifact: InterviewArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      const updated = await saveProductFactoryInterview(routeRunId, artifact);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const confirmInterview = async (artifact: InterviewArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      await saveProductFactoryInterview(routeRunId, { ...artifact, confirmed: true });
      const updated = await confirmProductFactoryInterview(routeRunId);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const saveBlueprint = async (artifact: BlueprintArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      const updated = await saveProductFactoryBlueprint(routeRunId, artifact);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const confirmBlueprint = async (artifact: BlueprintArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      await saveProductFactoryBlueprint(routeRunId, { ...artifact, confirmed: true });
      const updated = await confirmProductFactoryBlueprint(routeRunId);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const generateTaskDraft = async () => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      const updated = await generateProductFactoryTaskDraft(routeRunId);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const saveTaskDraft = async (artifact: TaskDraftArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      const updated = await saveProductFactoryTaskDraft(routeRunId, artifact);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const confirmTaskDraft = async (artifact: TaskDraftArtifact) => {
    if (!routeRunId) return;
    setSavingArtifact(true);
    try {
      const saved = await saveProductFactoryTaskDraft(routeRunId, artifact);
      const savedDraft = saved.taskDraft as TaskDraftArtifact;
      setWorkspaceRun(saved);
      const updated = await confirmProductFactoryTaskDraft(routeRunId, savedDraft.revision);
      setWorkspaceRun(updated);
      setArtifactDirty(false);
    } finally {
      setSavingArtifact(false);
    }
  };

  const persistDraft = async (nextDraft: ProductIdeaDraft): Promise<ProductIdeaDraft> => {
    const normalized = saveProductIdeaDraft(nextDraft);
    setDraft(normalized);
    if (!runId || apiUnavailable) {
      try {
        const run = await createProductFactoryRun(normalized);
        setRunId(run.id);
        saveProductFactoryRunId(run.id);
        setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)]);
        setApiUnavailable(false);
      } catch {
        // Keep the local draft so the user can retry when the backend is available.
        setApiUnavailable(true);
      }
    }
    return normalized;
  };

  return (
    <main className={styles.page} data-testid='product-factory-page'>
      <div className={styles.content}>
        {showIdeaForm ? (
          <IdeaDraftForm
            initialDraft={draft}
            onBack={() => void navigate('/product-factory')}
            onSave={persistDraft}
            onStartAnalysis={async (nextDraft) => {
              const normalized = saveProductIdeaDraft(nextDraft);
              setDraft(normalized);
              // Starting a new product always creates a new run, regardless of
              // whether another run is selected in the home page or storage.
              const run = await createProductFactoryRun(normalized);
              setRunId(run.id);
              saveProductFactoryRunId(run.id);
              setRuns((current) => [run, ...current.filter((item) => item.id !== run.id)]);
              setApiUnavailable(false);
              void navigate('/product-factory');
            }}
          />
        ) : showWorkspace ? (
          <RunWorkspace
            run={workspaceRun}
            loading={workspaceLoading}
            error={workspaceError}
            onBack={() => void navigate('/product-factory')}
            onRetry={() => void loadWorkspace()}
            onInterviewChange={(artifact) => {
              setArtifactDirty(true);
              setWorkspaceRun((current) => (current ? { ...current, interview: artifact } : current));
            }}
            onInterviewSave={saveInterview}
            onInterviewConfirm={confirmInterview}
            onBlueprintChange={(artifact) => {
              setArtifactDirty(true);
              setWorkspaceRun((current) => (current ? { ...current, blueprint: artifact } : current));
            }}
            onBlueprintSave={saveBlueprint}
            onBlueprintConfirm={confirmBlueprint}
            onTaskDraftChange={(artifact) => {
              setArtifactDirty(true);
              setWorkspaceRun((current) => (current ? { ...current, taskDraft: artifact } : current));
            }}
            hasUnsavedChanges={artifactDirty}
            onPlanningApplied={(updated) => {
              if (updated.id === routeRunId) {
                setWorkspaceRun(updated);
                setArtifactDirty(false);
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
            onApproveReview={() => applyReview('approve')}
            onRequestChanges={(feedback) => applyReview('request_changes', feedback)}
            onContinueReview={continueReview}
            delivery={delivery}
          />
        ) : (
          <FactoryHome
            draft={draft}
            runs={runs}
            loadingRuns={loadingRuns}
            apiUnavailable={apiUnavailable}
            onRetry={() => void loadRuns()}
            onCreate={() => {
              setDraft({ name: '', idea: '', targetUser: '', problem: '', expectedOutput: '', workspacePath: '' });
              setRunId('');
              void navigate('/product-factory/new');
            }}
            onContinue={() => void navigate(runs[0]?.id ? `/product-factory/${runs[0].id}` : '/product-factory/new')}
          />
        )}
      </div>
    </main>
  );
};

export default ProductFactoryPage;
