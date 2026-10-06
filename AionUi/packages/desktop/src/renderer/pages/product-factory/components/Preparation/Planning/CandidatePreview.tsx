/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Descriptions, Tag } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import { asRecord } from '../../../handoffPreview';
import type { PlanningAttempt } from '../../../types';

const values = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
const text = (value: unknown): string => (typeof value === 'string' ? value : '');

export default function CandidatePreview({ attempt }: { attempt: PlanningAttempt }) {
  const { t } = useTranslation();
  const candidate = attempt.candidate;
  if (!candidate) return null;
  const list =
    attempt.phase === 'interview'
      ? candidate.questions
      : attempt.phase === 'blueprint'
        ? candidate.sections
        : candidate.tasks;
  return (
    <section aria-label={t('productFactory.planning.candidate')} className='min-w-0'>
      <h3>{t('productFactory.planning.candidate')}</h3>
      <p className='text-13px text-t-secondary'>{t('productFactory.planning.applyHint')}</p>
      {Array.isArray(list) &&
        list.map((value, index) => {
          const item = asRecord(value);
          return (
            <div key={`${text(item.id)}-${index}`} className='mb-12px rounded-8px border border-border-2 p-12px'>
              <strong>{text(item.question) || text(item.title)}</strong>
              <Tag className='ms-8px'>{text(item.id)}</Tag>
              {text(item.reason) && <p>{text(item.reason)}</p>}
              {(text(item.content) || text(item.description)) && (
                <p className='whitespace-pre-wrap'>{text(item.content) || text(item.description)}</p>
              )}
              {attempt.phase === 'task_graph' && (
                <Descriptions
                  size='small'
                  column={1}
                  data={[
                    {
                      label: t('productFactory.taskDraft.scopeField'),
                      value:
                        item.execution_scope === 'project_integration'
                          ? t('productFactory.taskDraft.scopes.projectIntegration')
                          : t('productFactory.taskDraft.scopes.taskWorkspace'),
                    },
                    {
                      label: t('productFactory.taskDraft.requirementsField'),
                      value: values(item.requirement_ids).join(', '),
                    },
                    {
                      label: t('productFactory.taskDraft.dependenciesField'),
                      value: values(item.blocked_by).join(', '),
                    },
                  ]}
                />
              )}
              {values(item.acceptance_criteria).length > 0 && (
                <ul>
                  {values(item.acceptance_criteria).map((criterion, i) => (
                    <li key={i}>{criterion}</li>
                  ))}
                </ul>
              )}
            </div>
          );
        })}
      {attempt.phase === 'blueprint' && Array.isArray(candidate.requirements) && (
        <>
          <h4>{t('productFactory.blueprint.requirements')}</h4>
          {candidate.requirements.map((value, index) => {
            const requirement = asRecord(value);
            return (
              <div key={`${text(requirement.id)}-${index}`} className='mb-12px'>
                <Tag>{text(requirement.id)}</Tag> {text(requirement.title)}
                <ul>
                  {values(requirement.acceptance_criteria).map((criterion, i) => (
                    <li key={i}>{criterion}</li>
                  ))}
                </ul>
              </div>
            );
          })}
        </>
      )}
      {values(candidate.risks).length > 0 && (
        <>
          <h4>{t('productFactory.blueprint.risks')}</h4>
          <ul>
            {values(candidate.risks).map((item, i) => (
              <li key={i}>{item}</li>
            ))}
          </ul>
        </>
      )}
      {values(candidate.open_questions).length > 0 && (
        <>
          <h4>{t('productFactory.blueprint.openQuestions')}</h4>
          <ul>
            {values(candidate.open_questions).map((item, i) => (
              <li key={i}>{item}</li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}
