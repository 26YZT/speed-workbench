/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */
import { normalizeRun, request } from '../../client';
import { asRecord } from '../../handoffPreview';
import type {
  CodeManifest,
  IterateVersionInput,
  ProductVersion,
  ProductVersions,
  SnapshotVersionInput,
  SourceManifest,
  VersionOperation,
  VersionProduct,
  VersionReceipt,
} from '../../types';

const invalid = () => new Error('PRODUCT_FACTORY_INVALID_VERSION_RESPONSE');
const text = (record: Record<string, unknown>, key: string): string => {
  if (typeof record[key] !== 'string') throw invalid();
  return record[key] as string;
};
const optionalText = (record: Record<string, unknown>, key: string): string | undefined => {
  if (record[key] == null) return undefined;
  return text(record, key);
};
const number = (record: Record<string, unknown>, key: string): number => {
  if (!Number.isSafeInteger(record[key]) || Number(record[key]) < 0) throw invalid();
  return record[key] as number;
};
/** Validate the code-only manifest before presenting selectable file hashes. */
export function parseCodeManifest(value: unknown): CodeManifest {
  const raw = asRecord(value);
  if (raw.version !== 1 || raw.scope !== 'code_only' || !Array.isArray(raw.files)) throw invalid();
  const paths = new Set<string>();
  const files = raw.files.map((fileValue) => {
    const file = asRecord(fileValue);
    const path = text(file, 'path');
    const sha256 = text(file, 'sha256');
    if (
      !path ||
      path.startsWith('/') ||
      path.includes('\\') ||
      path.includes('\0') ||
      path.split('/').some((part) => !part || part === '..' || part === '.') ||
      paths.has(path) ||
      !/^[a-f0-9]{64}$/.test(sha256)
    )
      throw invalid();
    paths.add(path);
    return { path, sha256, sizeBytes: number(file, 'size_bytes') };
  });
  return { version: 1, scope: 'code_only', files, totalBytes: number(raw, 'total_bytes') };
}
function product(value: unknown): VersionProduct {
  const raw = asRecord(value);
  return {
    id: text(raw, 'id'),
    name: text(raw, 'name'),
    activeVersionId: optionalText(raw, 'active_version_id'),
    revision: number(raw, 'revision'),
    createdAt: number(raw, 'created_at'),
    updatedAt: number(raw, 'updated_at'),
  };
}
function parseVersion(value: unknown): ProductVersion {
  const raw = asRecord(value);
  const state = text(raw, 'state');
  if (!['copying', 'working', 'sealed', 'failed'].includes(state)) throw invalid();
  return {
    id: text(raw, 'id'),
    productId: text(raw, 'product_id'),
    runId: text(raw, 'run_id'),
    parentRunId: optionalText(raw, 'parent_run_id'),
    parentVersionId: optionalText(raw, 'parent_version_id'),
    versionNo: number(raw, 'version_no'),
    state: state as ProductVersion['state'],
    snapshotPath: optionalText(raw, 'snapshot_path'),
    manifest: raw.manifest == null ? undefined : parseCodeManifest(raw.manifest),
    changeRequest: optionalText(raw, 'change_request'),
    errorCode: optionalText(raw, 'error_code'),
    createdAt: number(raw, 'created_at'),
    updatedAt: number(raw, 'updated_at'),
    sealedAt: raw.sealed_at == null ? undefined : number(raw, 'sealed_at'),
  };
}
function operation(value: unknown): VersionOperation {
  const raw = asRecord(value);
  const kind = text(raw, 'kind');
  const state = text(raw, 'state');
  if (!['snapshot', 'iterate'].includes(kind) || !['reserved', 'copying', 'complete', 'failed'].includes(state))
    throw invalid();
  return {
    id: text(raw, 'id'),
    kind: kind as VersionOperation['kind'],
    state: state as VersionOperation['state'],
    versionId: text(raw, 'version_id'),
    idempotencyKey: text(raw, 'idempotency_key'),
    errorCode: optionalText(raw, 'error_code'),
    createdAt: number(raw, 'created_at'),
    updatedAt: number(raw, 'updated_at'),
  };
}
function receipt(value: unknown): VersionReceipt {
  const raw = asRecord(value);
  const result = {
    product: product(raw.product),
    version: parseVersion(raw.version),
    operation: operation(raw.operation),
  };
  if (result.version.productId !== result.product.id || result.operation.versionId !== result.version.id)
    throw invalid();
  return result;
}
const path = (runId: string) => `/api/product-factory/runs/${encodeURIComponent(runId)}/versions`;

/** Read version history and durable operation receipts without retrying writes. */
export async function getProductVersions(runId: string): Promise<ProductVersions> {
  const raw = asRecord(await request<unknown>(path(runId), { cache: 'no-store' }));
  if (!Array.isArray(raw.versions) || !Array.isArray(raw.operations)) throw invalid();
  const result = {
    product: raw.product == null ? undefined : product(raw.product),
    currentVersionId: optionalText(raw, 'current_version_id'),
    versions: raw.versions.map(parseVersion),
    operations: raw.operations.map(operation),
  };
  if (!result.product && (result.versions.length > 0 || result.operations.length > 0 || result.currentVersionId))
    throw invalid();
  if (result.product && result.versions.some((version) => version.productId !== result.product?.id)) throw invalid();
  if (
    result.currentVersionId &&
    !result.versions.some((version) => version.id === result.currentVersionId && version.runId === runId)
  )
    throw invalid();
  return result;
}
/** Inspect the backend's eligible code files and their current hashes. */
export async function getSourceManifest(runId: string): Promise<SourceManifest> {
  const raw = asRecord(await request<unknown>(`${path(runId)}/source-manifest`, { cache: 'no-store' }));
  if (!Array.isArray(raw.warnings) || !raw.warnings.every((warning) => typeof warning === 'string')) throw invalid();
  return {
    runId: text(raw, 'run_id'),
    planRevision: number(raw, 'plan_revision'),
    manifest: parseCodeManifest(raw.manifest),
    excludedCount: number(raw, 'excluded_count'),
    warnings: raw.warnings,
  };
}
/** Save an explicitly reviewed selection with its original idempotency key. */
export async function snapshotProductVersion(runId: string, input: SnapshotVersionInput): Promise<VersionReceipt> {
  return receipt(await request<unknown>(`${path(runId)}/snapshot`, { method: 'POST', body: JSON.stringify(input) }));
}
/** Create an independent requirement draft from sealed code, without starting models. */
export async function iterateProductVersion(runId: string, input: IterateVersionInput) {
  const raw = asRecord(
    await request<unknown>(`${path(runId)}/iterate`, { method: 'POST', body: JSON.stringify(input) })
  );
  const newRun = asRecord(raw.run);
  if (typeof newRun.id !== 'string' || newRun.status !== 'draft') throw invalid();
  const result = receipt(raw);
  if (result.version.runId !== newRun.id) throw invalid();
  return { ...result, run: normalizeRun(newRun as Parameters<typeof normalizeRun>[0]) };
}
/** Select sealed code using the current product revision; business data stays separate. */
export async function activateProductVersion(runId: string, versionId: string, revision: number) {
  const raw = asRecord(
    await request<unknown>(`${path(runId)}/activate`, {
      method: 'POST',
      body: JSON.stringify({ version_id: versionId, expected_product_revision: revision }),
    })
  );
  if (raw.scope !== 'code_only') throw invalid();
  const selected = parseVersion(raw.version);
  if (selected.state !== 'sealed') throw invalid();
  const selectedProduct = product(raw.product);
  if (
    selected.id !== versionId ||
    selected.productId !== selectedProduct.id ||
    selectedProduct.activeVersionId !== selected.id
  )
    throw invalid();
  return {
    product: selectedProduct,
    version: selected,
    workspacePath: text(raw, 'workspace_path'),
    scope: 'code_only' as const,
  };
}
