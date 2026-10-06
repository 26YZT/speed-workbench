/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { ProductFactoryHttpError } from '../../client';
import type { PendingVersionWrite, ProductFactoryRun, ProductVersions, VersionReceipt } from '../../types';
import {
  activateProductVersion,
  getProductVersions,
  iterateProductVersion,
  snapshotProductVersion,
} from './versionClient';

const storageKey = (runId: string) => `product-factory.version-write.${runId}`;
function restorePending(runId: string): PendingVersionWrite | undefined {
  try {
    const raw = JSON.parse(window.localStorage.getItem(storageKey(runId)) ?? 'null') as PendingVersionWrite | null;
    if (!raw || !['snapshot', 'iterate'].includes(raw.kind) || typeof raw.input?.idempotency_key !== 'string')
      return undefined;
    if (
      raw.kind === 'snapshot' &&
      (!Array.isArray(raw.input.files) || !Number.isSafeInteger(raw.input.expected_plan_revision))
    )
      return undefined;
    if (
      raw.kind === 'iterate' &&
      (typeof raw.input.change_request !== 'string' ||
        typeof raw.input.source_version_id !== 'string' ||
        !Number.isSafeInteger(raw.input.expected_product_revision))
    )
      return undefined;
    return raw;
  } catch {
    return undefined;
  }
}
function rememberPending(runId: string, pending?: PendingVersionWrite) {
  try {
    if (pending) window.localStorage.setItem(storageKey(runId), JSON.stringify(pending));
    else window.localStorage.removeItem(storageKey(runId));
  } catch {
    /* Backend operation receipts remain authoritative if local storage is unavailable. */
  }
}

/** Restore receipts and poll local copy progress with GET only. */
export function useVersions(runId: string) {
  const [history, setHistory] = useState<ProductVersions>();
  const [pending, setPending] = useState<PendingVersionWrite | undefined>(() => restorePending(runId));
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [readFailed, setReadFailed] = useState(false);
  const [errorCode, setErrorCode] = useState<string>();
  const [error, setError] = useState<'readError' | 'operationError'>();
  const [submittedKey, setSubmittedKey] = useState<string>();
  const mounted = useRef(true);
  const action = useRef(false);
  const reads = useRef(false);
  const generation = useRef(0);
  const pendingRef = useRef(pending);
  pendingRef.current = pending;

  const refresh = useCallback(async () => {
    if (reads.current) return undefined;
    reads.current = true;
    const started = generation.current;
    try {
      const loaded = await getProductVersions(runId);
      if (!mounted.current || started !== generation.current) return undefined;
      setHistory(loaded);
      setReadFailed(false);
      setError((current) => (current === 'readError' ? undefined : current));
      const remembered = pendingRef.current;
      if (
        remembered &&
        loaded.operations.some(
          (operation) => operation.idempotencyKey === remembered.input.idempotency_key && operation.state === 'complete'
        )
      ) {
        setPending(undefined);
        pendingRef.current = undefined;
        rememberPending(runId);
      }
      return loaded;
    } catch {
      if (mounted.current) {
        setReadFailed(true);
        setError('readError');
      }
      return undefined;
    } finally {
      reads.current = false;
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
  const copying =
    history?.versions.some((version) => version.id === history.currentVersionId && version.state === 'copying') ||
    Boolean(
      pending &&
      history?.operations.some(
        (operation) =>
          operation.idempotencyKey === pending.input.idempotency_key &&
          ['reserved', 'copying'].includes(operation.state)
      )
    );
  useEffect(() => {
    if (!copying || busy) return;
    const poll = () => {
      if (document.visibilityState === 'visible') void refresh();
    };
    const timer = window.setInterval(poll, 5000);
    document.addEventListener('visibilitychange', poll);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', poll);
    };
  }, [copying, busy, refresh]);

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
        setError('operationError');
        setErrorCode(caught instanceof ProductFactoryHttpError ? caught.code : undefined);
      }
      await refresh();
      return undefined;
    } finally {
      action.current = false;
      if (mounted.current) setBusy(false);
    }
  };
  const submit = async (
    write: PendingVersionWrite
  ): Promise<(VersionReceipt & { run?: ProductFactoryRun }) | undefined> =>
    execute(async () => {
      setSubmittedKey(write.input.idempotency_key);
      setPending(write);
      pendingRef.current = write;
      rememberPending(runId, write);
      const receipt: VersionReceipt & { run?: ProductFactoryRun } =
        write.kind === 'snapshot'
          ? await snapshotProductVersion(runId, write.input)
          : await iterateProductVersion(runId, write.input);
      if (mounted.current)
        setHistory((current) => ({
          product: receipt.product,
          currentVersionId: receipt.version.runId === runId ? receipt.version.id : current?.currentVersionId,
          versions: [
            receipt.version,
            ...(current?.versions ?? []).filter((version) => version.id !== receipt.version.id),
          ],
          operations: [
            receipt.operation,
            ...(current?.operations ?? []).filter((operation) => operation.id !== receipt.operation.id),
          ],
        }));
      if (mounted.current && receipt.operation.state === 'complete') {
        setPending(undefined);
        pendingRef.current = undefined;
        rememberPending(runId);
      }
      await refresh();
      return receipt;
    });
  const retryPending = () => (pendingRef.current ? submit(pendingRef.current) : Promise.resolve(undefined));
  const newRequest = () => {
    setPending(undefined);
    pendingRef.current = undefined;
    rememberPending(runId);
    setError(undefined);
    setErrorCode(undefined);
    setSubmittedKey(undefined);
  };
  const activate = (versionId: string, revision: number) =>
    execute(async () => {
      const receipt = await activateProductVersion(runId, versionId, revision);
      if (mounted.current) setHistory((current) => (current ? { ...current, product: receipt.product } : current));
      await refresh();
      return receipt;
    });
  const acknowledgeReceipt = useCallback(() => setSubmittedKey(undefined), []);
  return {
    history,
    pending,
    submittedKey,
    loading,
    busy,
    readFailed,
    error,
    errorCode,
    copying,
    refresh,
    submit,
    retryPending,
    newRequest,
    acknowledgeReceipt,
    activate,
  };
}
