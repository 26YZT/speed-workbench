/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Button, Input, Switch, Tag } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import type { InterviewArtifact } from '../../types';
import styles from '../../ProductFactory.module.css';

type InterviewPanelProps = {
  artifact: InterviewArtifact;
  onChange: (artifact: InterviewArtifact) => void;
  onSave: () => void;
  onConfirm: () => void;
  saving: boolean;
};

const InterviewPanel: React.FC<InterviewPanelProps> = ({ artifact, onChange, onSave, onConfirm, saving }) => {
  const { t } = useTranslation();
  const updateQuestion = (id: string, patch: Partial<InterviewArtifact['questions'][number]>) => {
    onChange({
      ...artifact,
      questions: artifact.questions.map((question) => (question.id === id ? { ...question, ...patch } : question)),
      updatedAt: Date.now(),
    });
  };
  const canConfirm =
    artifact.summary.trim().length > 0 &&
    artifact.questions.every((question) => question.answer.trim() || question.notSure);

  return (
    <section className='rounded-12px border border-border-2 bg-bg-2 p-20px'>
      <div className='mb-18px'>
        <h2 className='m-0 text-20px font-650 text-t-primary'>{t('productFactory.interview.title')}</h2>
        <p className='m-0 mt-8px text-13px leading-20px text-t-secondary'>
          {t('productFactory.interview.description')}
        </p>
      </div>
      <Tag className='mb-12px'>
        {artifact.generatedBy === 'model'
          ? t('productFactory.planning.modelSource')
          : t('productFactory.planning.ruleSource')}
      </Tag>
      <div className='flex flex-col gap-16px'>
        {artifact.questions.slice(0, 5).map((question) => (
          <div className={styles.field} key={question.id}>
            <span className={styles.label}>{question.prompt}</span>
            {question.reason && <p className='m-0 text-13px text-t-secondary'>{question.reason}</p>}
            <Input.TextArea
              data-testid={`product-factory-interview-answer-${question.id}`}
              value={question.answer}
              disabled={saving || question.notSure}
              autoSize={{ minRows: 2, maxRows: 5 }}
              onChange={(answer) => updateQuestion(question.id, { answer })}
            />
            <label className='inline-flex items-center gap-8px text-13px text-t-secondary'>
              <Switch
                checked={question.notSure}
                disabled={saving}
                onChange={(notSure) => updateQuestion(question.id, { notSure, answer: notSure ? '' : question.answer })}
              />
              {t('productFactory.interview.notSure')}
            </label>
          </div>
        ))}
        <label className={styles.field}>
          <span className={styles.label}>{t('productFactory.interview.summaryLabel')}</span>
          <Input.TextArea
            data-testid='product-factory-interview-summary'
            value={artifact.summary}
            disabled={saving}
            autoSize={{ minRows: 3, maxRows: 7 }}
            onChange={(summary) => onChange({ ...artifact, summary, updatedAt: Date.now() })}
          />
        </label>
      </div>
      <div className='mt-20px flex flex-wrap justify-end gap-10px border-t border-border-2 pt-16px'>
        <Button loading={saving} onClick={onSave}>
          {t('productFactory.interview.save')}
        </Button>
        <Button type='primary' disabled={!canConfirm || saving} loading={saving} onClick={onConfirm}>
          {t('productFactory.interview.confirm')}
        </Button>
      </div>
    </section>
  );
};

export default InterviewPanel;
