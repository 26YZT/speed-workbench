/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useRef, useState } from 'react';
import { Button, Input } from '@arco-design/web-react';
import { ArrowLeft, CheckOne, Save } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { ProductIdeaDraft } from '../types';
import styles from '../ProductFactory.module.css';
import { hasProductIdeaDraft } from '../draftStore';

type IdeaDraftFormProps = {
  initialDraft: ProductIdeaDraft;
  onBack: () => void;
  onSave: (draft: ProductIdeaDraft) => ProductIdeaDraft | Promise<ProductIdeaDraft>;
  onStartAnalysis: (draft: ProductIdeaDraft) => Promise<void>;
  onDraftChange?: (draft: ProductIdeaDraft) => void;
  localSaveError?: boolean;
};

type DraftErrors = Partial<Record<'name' | 'idea' | 'budget', string>>;

const IdeaDraftForm: React.FC<IdeaDraftFormProps> = ({
  initialDraft,
  onBack,
  onSave,
  onStartAnalysis,
  onDraftChange,
  localSaveError = false,
}) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(initialDraft);
  const [errors, setErrors] = useState<DraftErrors>({});
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | undefined>();
  const busy = saving || submitting;
  const action = useRef(false);

  const setField = <K extends keyof ProductIdeaDraft>(key: K, value: ProductIdeaDraft[K]) => {
    if (busy || action.current) return;
    const next = { ...draft, [key]: value };
    setDraft(next);
    onDraftChange?.(next);
    setSaved(false);
    if (key === 'name' || key === 'idea') {
      setErrors((current) => ({ ...current, [key]: undefined }));
    }
  };

  const validate = (): boolean => {
    const nextErrors: DraftErrors = {};
    if (!draft.name.trim()) nextErrors.name = t('productFactory.form.validation.nameRequired');
    if (!draft.idea.trim()) nextErrors.idea = t('productFactory.form.validation.ideaRequired');
    const budget = (draft.budgetInput ?? (draft.budgetUsd === undefined ? '' : String(draft.budgetUsd))).trim();
    if (budget && (!Number.isFinite(Number(budget)) || Number(budget) <= 0))
      nextErrors.budget = t('productFactory.form.validation.budgetInvalid');
    setErrors(nextErrors);
    return Object.keys(nextErrors).length === 0;
  };

  const handleSave = async () => {
    if (busy || action.current) return;
    if (!validate()) return;
    action.current = true;
    setSaving(true);
    setSubmitError(undefined);
    try {
      const normalized = await onSave(draft);
      setDraft(normalized);
      setSaved(true);
    } catch {
      setSubmitError(t('productFactory.form.saveError'));
    } finally {
      action.current = false;
      setSaving(false);
    }
  };

  const handleStartAnalysis = async () => {
    if (busy || action.current) return;
    if (!validate()) return;
    action.current = true;
    setSubmitting(true);
    setSubmitError(undefined);
    try {
      await onStartAnalysis(draft);
      setSaved(true);
    } catch {
      setSubmitError(t('productFactory.form.startError'));
    } finally {
      action.current = false;
      setSubmitting(false);
    }
  };

  return (
    <div className='mx-auto w-full max-w-960px'>
      <Button type='text' icon={<ArrowLeft size={16} />} onClick={onBack} disabled={busy}>
        {t('productFactory.form.back')}
      </Button>

      <header className='mt-16px'>
        <h1 className='m-0 text-26px font-650 leading-36px text-t-primary'>{t('productFactory.form.title')}</h1>
        <p className='m-0 mt-8px text-14px leading-22px text-t-secondary'>{t('productFactory.form.description')}</p>
      </header>

      <section className='mt-22px rounded-12px border border-border-2 bg-bg-2 p-22px max-640px:p-16px'>
        <div className={styles.formGrid}>
          <label className={styles.field}>
            <span className={styles.label}>{t('productFactory.form.nameLabel')}</span>
            <Input
              disabled={busy}
              data-testid='product-factory-name'
              value={draft.name}
              status={errors.name ? 'error' : undefined}
              placeholder={t('productFactory.form.namePlaceholder')}
              onChange={(value) => setField('name', value)}
              allowClear
            />
            {errors.name && <span className={styles.error}>{errors.name}</span>}
          </label>

          <label className={`${styles.field} ${styles.fullWidth}`}>
            <span className={styles.label}>{t('productFactory.form.ideaLabel')}</span>
            <Input.TextArea
              disabled={busy}
              data-testid='product-factory-idea'
              value={draft.idea}
              status={errors.idea ? 'error' : undefined}
              placeholder={t('productFactory.form.ideaPlaceholder')}
              onChange={(value) => setField('idea', value)}
              autoSize={{ minRows: 4, maxRows: 8 }}
              maxLength={2000}
              showWordLimit
            />
            {errors.idea && <span className={styles.error}>{errors.idea}</span>}
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t('productFactory.form.targetUserLabel')}</span>
            <Input
              disabled={busy}
              value={draft.targetUser}
              placeholder={t('productFactory.form.targetUserPlaceholder')}
              onChange={(value) => setField('targetUser', value)}
              allowClear
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t('productFactory.form.expectedOutputLabel')}</span>
            <Input
              disabled={busy}
              value={draft.expectedOutput}
              placeholder={t('productFactory.form.expectedOutputPlaceholder')}
              onChange={(value) => setField('expectedOutput', value)}
              allowClear
            />
          </label>

          <label className={`${styles.field} ${styles.fullWidth}`}>
            <span className={styles.label}>{t('productFactory.form.problemLabel')}</span>
            <Input.TextArea
              disabled={busy}
              value={draft.problem}
              placeholder={t('productFactory.form.problemPlaceholder')}
              onChange={(value) => setField('problem', value)}
              autoSize={{ minRows: 2, maxRows: 5 }}
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t('productFactory.form.workspaceLabel')}</span>
            <Input
              disabled={busy}
              data-testid='product-factory-workspace'
              value={draft.workspacePath}
              placeholder={t('productFactory.form.workspacePlaceholder')}
              onChange={(value) => setField('workspacePath', value)}
              allowClear
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t('productFactory.form.budgetLabel')}</span>
            <Input
              disabled={busy}
              data-testid='product-factory-budget'
              value={draft.budgetInput ?? (draft.budgetUsd === undefined ? '' : String(draft.budgetUsd))}
              status={errors.budget ? 'error' : undefined}
              aria-invalid={Boolean(errors.budget)}
              placeholder={t('productFactory.form.budgetPlaceholder')}
              inputMode='decimal'
              onChange={(value) => {
                if (busy) return;
                const parsed = Number(value);
                const next = {
                  ...draft,
                  budgetInput: value,
                  budgetUsd: value.trim() && Number.isFinite(parsed) && parsed > 0 ? parsed : undefined,
                };
                setDraft(next);
                setSaved(false);
                onDraftChange?.(next);
                setErrors((current) => ({
                  ...current,
                  budget:
                    value.trim() && (!Number.isFinite(parsed) || parsed <= 0)
                      ? t('productFactory.form.validation.budgetInvalid')
                      : undefined,
                }));
              }}
              allowClear
            />
            {errors.budget && (
              <span className={styles.error} role='alert'>
                {errors.budget}
              </span>
            )}
            <span className={styles.hint}>{t('productFactory.form.budgetHint')}</span>
          </label>
        </div>

        <div className='mt-22px flex flex-wrap items-center justify-end gap-10px border-t border-border-2 pt-18px'>
          {!saved && !localSaveError && onDraftChange && hasProductIdeaDraft(draft) && (
            <span className='me-auto text-12px text-t-secondary' role='status'>
              {t('productFactory.form.autoSaved')}
            </span>
          )}
          {saved && (
            <span className='me-auto inline-flex items-center gap-6px text-13px text-success-6' role='status'>
              <CheckOne size={15} />
              {t('productFactory.form.saved')}
            </span>
          )}
          {submitError && (
            <span className={styles.error} role='alert'>
              {submitError}
            </span>
          )}
          <Button icon={<Save size={16} />} disabled={submitting} loading={saving} onClick={() => void handleSave()}>
            {t('productFactory.form.saveDraft')}
          </Button>
          <Button type='primary' loading={submitting} disabled={saving} onClick={() => void handleStartAnalysis()}>
            {submitting ? t('productFactory.form.startingAnalysis') : t('productFactory.form.startAnalysis')}
          </Button>
        </div>
      </section>
    </div>
  );
};

export default IdeaDraftForm;
