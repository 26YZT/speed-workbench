/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  applyProductFactoryPlanning,
  cancelProductFactoryPlanning,
  getProductFactoryCostReport,
  listProductFactoryPlanning,
  startProductFactoryPlanning,
  ProductFactoryHttpError,
} from '../../../client';
import type { PlanningAttempt, PlanningPhase, ProductFactoryCostReport, ProductFactoryRun } from '../../../types';

export const isPlanningActive = (attempt: PlanningAttempt): boolean =>
  ['reserved', 'preparing', 'running'].includes(attempt.state);

/** Restore durable attempts and poll only their read endpoints while visible. */
export function usePlanning(runId: string) {
  const [attempts, setAttempts] = useState<PlanningAttempt[]>([]);
  const [report, setReport] = useState<ProductFactoryCostReport>();
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [errorCode, setErrorCode] = useState<string>();
  const [readFailed, setReadFailed] = useState(false);
  const mounted = useRef(true);
  const action = useRef(false);
  const read = useRef(false);
  const generation = useRef(0);

  const refresh = useCallback(async () => {
    if (read.current) return;
    read.current = true;
    const startedGeneration = generation.current;
    try {
      const [attemptResult, costResult] = await Promise.allSettled([
        listProductFactoryPlanning(runId),
        getProductFactoryCostReport(runId),
      ]);
      if (!mounted.current || startedGeneration !== generation.current) return;
      if (attemptResult.status === 'rejected') {
        setReadFailed(true);
        setError('readError');
      } else {
        setAttempts(attemptResult.value);
        setReadFailed(false);
        setError((current) => (current === 'readError' ? undefined : current));
      }
      setReport(costResult.status === 'fulfilled' ? costResult.value : undefined);
    } finally {
      read.current = false;
      if (mounted.current) setLoading(false);
    }
  }, [runId]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  const hasActive = attempts.some(isPlanningActive);
  useEffect(() => {
    if (!hasActive || busy) return;
    const poll = () => {
      if (document.visibilityState === 'visible') void refresh();
    };
    const timer = window.setInterval(poll, 5000);
    document.addEventListener('visibilitychange', poll);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', poll);
    };
  }, [hasActive, busy, refresh]);

  const execute = async <T>(operation: () => Promise<T>): Promise<T | undefined> => {
    if (action.current) return undefined;
    action.current = true;
    generation.current += 1;
    setBusy(true);
    setError(undefined);
    setErrorCode(undefined);
    try {
      return await operation();
    } catch (caught) {
      if (mounted.current) {
        setError('requestError');
        setErrorCode(caught instanceof ProductFactoryHttpError ? caught.code : undefined);
        setReadFailed(true);
      }
      // A lost response is recovered with reads only. Never resend a model call.
      await refresh();
      return undefined;
    } finally {
      action.current = false;
      if (mounted.current) setBusy(false);
    }
  };

  const start = async (phase: PlanningPhase, revision: number, assistantId: string, model: string) =>
    execute(async () => {
      const attempt = await startProductFactoryPlanning(runId, {
        phase,
        expectedPlanRevision: revision,
        idempotencyKey: window.crypto.randomUUID(),
        assistantId,
        model,
      });
      if (mounted.current) setAttempts((current) => [attempt, ...current.filter((item) => item.id !== attempt.id)]);
      await refresh();
      return attempt;
    });

  const apply = async (attempt: PlanningAttempt, revision: number): Promise<ProductFactoryRun | undefined> =>
    execute(async () => {
      const run = await applyProductFactoryPlanning(runId, attempt.id, revision);
      if (mounted.current)
        setAttempts((current) =>
          current.map((item) => (item.id === attempt.id ? { ...item, state: 'applied' } : item))
        );
      await refresh();
      return run;
    });

  const cancel = async (attempt: PlanningAttempt) =>
    execute(async () => {
      const cancelled = await cancelProductFactoryPlanning(runId, attempt.id);
      if (mounted.current) setAttempts((current) => current.map((item) => (item.id === attempt.id ? cancelled : item)));
      await refresh();
    });

  return { attempts, report, loading, busy, error, errorCode, readFailed, hasActive, refresh, start, apply, cancel };
}
