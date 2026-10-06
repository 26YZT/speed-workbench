/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState } from 'react';
import { Button, Input, Tabs, Tag } from '@arco-design/web-react';
import { useTranslation } from 'react-i18next';
import type { BlueprintArtifact } from '../../types';
import styles from '../../ProductFactory.module.css';

type BlueprintReviewProps = {
  artifact: BlueprintArtifact;
  onChange: (artifact: BlueprintArtifact) => void;
  onSave: () => void;
  onConfirm: () => void;
  saving: boolean;
};

const BlueprintReview: React.FC<BlueprintReviewProps> = ({ artifact, onChange, onSave, onConfirm, saving }) => {
  const { t } = useTranslation();
  const [activeSection, setActiveSection] = useState(artifact.sections[0]?.id ?? '');
  const section = artifact.sections.find((item) => item.id === activeSection) ?? artifact.sections[0];
  const updateSection = (content: string) => {
    if (!section) return;
    onChange({
      ...artifact,
      confirmed: false,
      sections: artifact.sections.map((item) => (item.id === section.id ? { ...item, content } : item)),
      updatedAt: Date.now(),
    });
  };

  return (
    <section className='rounded-12px border border-border-2 bg-bg-2 p-20px'>
      <div className='mb-18px'>
        <h2 className='m-0 text-20px font-650 text-t-primary'>{t('productFactory.blueprint.title')}</h2>
        <p className='m-0 mt-8px text-13px leading-20px text-t-secondary'>
          {t('productFactory.blueprint.description')}
        </p>
      </div>
      <Tag className='mb-12px'>
        {artifact.generatedBy === 'model'
          ? t('productFactory.planning.modelSource')
          : t('productFactory.planning.ruleSource')}
      </Tag>
      <Tabs activeTab={activeSection} onChange={setActiveSection} type='line'>
        {artifact.sections.map((item) => (
          <Tabs.TabPane key={item.id} title={item.title} />
        ))}
      </Tabs>
      {section && (
        <label className={`${styles.field} mt-16px`}>
          <span className={styles.label}>{section.title}</span>
          <Input.TextArea
            disabled={saving}
            value={section.content}
            autoSize={{ minRows: 6, maxRows: 14 }}
            onChange={updateSection}
          />
        </label>
      )}
      <div className='mt-18px grid gap-14px max-720px:grid-cols-2'>
        <label className={styles.field}>
          <span className={styles.label}>{t('productFactory.blueprint.risks')}</span>
          <Input.TextArea
            disabled={saving}
            value={artifact.risks.join('\n')}
            autoSize={{ minRows: 3, maxRows: 7 }}
            onChange={(value) => onChange({ ...artifact, confirmed: false, risks: value.split('\n').filter(Boolean) })}
          />
        </label>
        <label className={styles.field}>
          <span className={styles.label}>{t('productFactory.blueprint.openQuestions')}</span>
          <Input.TextArea
            disabled={saving}
            value={artifact.openQuestions.join('\n')}
            autoSize={{ minRows: 3, maxRows: 7 }}
            onChange={(value) =>
              onChange({ ...artifact, confirmed: false, openQuestions: value.split('\n').filter(Boolean) })
            }
          />
        </label>
      </div>
      {artifact.requirements && artifact.requirements.length > 0 && (
        <div className='mt-18px'>
          <h3>{t('productFactory.blueprint.requirements')}</h3>
          {artifact.requirements.map((requirement, index) => (
            <div key={requirement.id} className='mb-14px'>
              <Tag>{requirement.id}</Tag>
              <Input
                className='mt-8px'
                aria-label={t('productFactory.blueprint.requirementTitle')}
                value={requirement.title}
                disabled={saving}
                onChange={(title) =>
                  onChange({
                    ...artifact,
                    confirmed: false,
                    requirements: artifact.requirements?.map((item, i) => (i === index ? { ...item, title } : item)),
                  })
                }
              />
              <Input.TextArea
                className='mt-8px'
                aria-label={t('productFactory.blueprint.requirementCriteria')}
                value={requirement.acceptanceCriteria.join('\n')}
                disabled={saving}
                onChange={(value) =>
                  onChange({
                    ...artifact,
                    confirmed: false,
                    requirements: artifact.requirements?.map((item, i) =>
                      i === index ? { ...item, acceptanceCriteria: value.split('\n').filter(Boolean) } : item
                    ),
                  })
                }
              />
            </div>
          ))}
        </div>
      )}
      <div className='mt-20px flex flex-wrap justify-end gap-10px border-t border-border-2 pt-16px'>
        <Button loading={saving} onClick={onSave}>
          {t('productFactory.blueprint.save')}
        </Button>
        <Button type='primary' disabled={saving || artifact.sections.length === 0} loading={saving} onClick={onConfirm}>
          {t('productFactory.blueprint.confirm')}
        </Button>
      </div>
    </section>
  );
};

export default BlueprintReview;
