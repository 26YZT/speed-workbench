/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useMemo, useState } from 'react';
import { Button, Card, Input, Select, Space, Tag } from '@arco-design/web-react';
import { Add, Delete, Down, Save, Up } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { TaskDraftArtifact, TaskDraftTask } from '../../types';
import styles from '../../ProductFactory.module.css';

type TaskDraftReviewProps = {
  artifact: TaskDraftArtifact;
  onChange: (artifact: TaskDraftArtifact) => void;
  onGenerate: () => void;
  onSave: () => void;
  onConfirm: () => void;
  saving: boolean;
};

const emptyTask = (index: number): TaskDraftTask => ({
  id: `task-${Date.now()}-${index}`,
  title: '',
  description: '',
  type: 'frontend',
  blockedBy: [],
  acceptanceCriteria: [''],
  suggestedRole: '',
  effort: 'medium',
  executionScope: 'task_workspace',
  requirementIds: [],
});

const TaskDraftReview: React.FC<TaskDraftReviewProps> = ({
  artifact,
  onChange,
  onGenerate,
  onSave,
  onConfirm,
  saving,
}) => {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState(artifact.tasks[0]?.id);
  const selectedIndex = Math.max(
    0,
    artifact.tasks.findIndex((task) => task.id === selectedId)
  );
  const selected = artifact.tasks[selectedIndex];
  const editingDisabled = saving || artifact.confirmed;
  const hasUnsavedChanges = useMemo(() => false, [artifact]);
  const updateTask = (patch: Partial<TaskDraftTask>) => {
    if (!selected) return;
    const tasks = artifact.tasks.map((task, index) => (index === selectedIndex ? { ...task, ...patch } : task));
    onChange({ ...artifact, tasks });
  };
  const move = (delta: -1 | 1) => {
    const next = selectedIndex + delta;
    if (!selected || next < 0 || next >= artifact.tasks.length) return;
    const tasks = [...artifact.tasks];
    [tasks[selectedIndex], tasks[next]] = [tasks[next], tasks[selectedIndex]];
    onChange({ ...artifact, tasks });
    setSelectedId(selected.id);
  };
  const remove = () => {
    if (!selected || artifact.tasks.some((task) => task.blockedBy.includes(selected.id))) return;
    const tasks = artifact.tasks.filter((task) => task.id !== selected.id);
    onChange({ ...artifact, tasks });
    setSelectedId(tasks[0]?.id);
  };
  if (!artifact.tasks.length) {
    return (
      <Card title={t('productFactory.taskDraft.title')}>
        <p>{t('productFactory.taskDraft.empty')}</p>
        <Button type='primary' onClick={onGenerate} loading={saving}>
          {t('productFactory.taskDraft.generate')}
        </Button>
      </Card>
    );
  }
  return (
    <Card
      title={t('productFactory.taskDraft.title')}
      extra={
        <Tag color={artifact.confirmed ? 'green' : 'arcoblue'}>
          {artifact.confirmed ? t('productFactory.taskDraft.confirmed') : t('productFactory.taskDraft.draft')}
        </Tag>
      }
    >
      <Tag className='mb-8px'>
        {artifact.generatedBy === 'model'
          ? t('productFactory.planning.modelSource')
          : t('productFactory.planning.ruleSource')}
      </Tag>
      <p className='mb-14px text-13px text-t-secondary'>{t('productFactory.taskDraft.description')}</p>
      {artifact.confirmed && (
        <p className='mb-14px text-13px text-t-secondary'>{t('productFactory.taskDraft.noAgent')}</p>
      )}
      <div className={styles.taskDraftLayout}>
        <div className={styles.taskDraftList} aria-label={t('productFactory.taskDraft.list')}>
          {artifact.tasks.map((task, index) => (
            <div
              className={`${styles.taskDraftItem} ${task.id === selected?.id ? styles.taskDraftItemActive : ''}`}
              key={task.id}
              onClick={() => setSelectedId(task.id)}
              role='button'
              tabIndex={0}
            >
              <span className='truncate'>{task.title || t('productFactory.taskDraft.untitled')}</span>
              <Tag size='small'>{index + 1}</Tag>
            </div>
          ))}
          {!artifact.confirmed && (
            <Button
              icon={<Add />}
              disabled={saving}
              onClick={() => {
                const task = emptyTask(artifact.tasks.length);
                onChange({ ...artifact, tasks: [...artifact.tasks, task] });
                setSelectedId(task.id);
              }}
            >
              {t('productFactory.taskDraft.add')}
            </Button>
          )}
        </div>
        {selected && (
          <div className={styles.taskDraftEditor}>
            <Space direction='vertical' size={12} style={{ width: '100%' }}>
              <Input
                value={selected.title}
                onChange={(title) => updateTask({ title })}
                disabled={editingDisabled}
                placeholder={t('productFactory.taskDraft.titleField')}
              />
              <Input.TextArea
                value={selected.description}
                onChange={(description) => updateTask({ description })}
                disabled={editingDisabled}
                placeholder={t('productFactory.taskDraft.descriptionField')}
                autoSize={{ minRows: 3, maxRows: 7 }}
              />
              <Space wrap>
                <Select value={selected.type} onChange={(type) => updateTask({ type })} disabled={editingDisabled}>
                  {['frontend', 'backend', 'data', 'test'].map((type) => (
                    <Select.Option key={type} value={type}>
                      {t(`productFactory.handoff.types.${type as TaskDraftTask['type']}`)}
                    </Select.Option>
                  ))}
                </Select>
                <Select
                  value={selected.effort}
                  onChange={(effort) => updateTask({ effort })}
                  disabled={editingDisabled}
                >
                  {['low', 'medium', 'high'].map((effort) => (
                    <Select.Option key={effort} value={effort}>
                      {t(`productFactory.handoff.efforts.${effort as TaskDraftTask['effort']}`)}
                    </Select.Option>
                  ))}
                </Select>
                <Input
                  value={selected.suggestedRole}
                  onChange={(suggestedRole) => updateTask({ suggestedRole })}
                  disabled={editingDisabled}
                  placeholder={t('productFactory.taskDraft.roleField')}
                />
              </Space>
              {artifact.version === 2 && (
                <>
                  <label className={styles.field}>
                    <span className={styles.label}>{t('productFactory.taskDraft.scopeField')}</span>
                    <Select
                      value={selected.executionScope}
                      disabled={editingDisabled}
                      aria-label={t('productFactory.taskDraft.scopeField')}
                      onChange={(executionScope: TaskDraftTask['executionScope']) => updateTask({ executionScope })}
                      options={[
                        { value: 'task_workspace', label: t('productFactory.taskDraft.scopes.taskWorkspace') },
                        {
                          value: 'project_integration',
                          label: t('productFactory.taskDraft.scopes.projectIntegration'),
                        },
                      ]}
                    />
                  </label>
                  <p className='m-0 text-13px text-t-secondary'>{t('productFactory.taskDraft.scopeHint')}</p>
                  <label className={styles.field}>
                    <span className={styles.label}>{t('productFactory.taskDraft.requirementsField')}</span>
                    <Input
                      value={selected.requirementIds?.join(', ') ?? ''}
                      disabled={editingDisabled}
                      placeholder={t('productFactory.taskDraft.requirementsPlaceholder')}
                      onChange={(value) =>
                        updateTask({
                          requirementIds: value
                            .split(',')
                            .map((item) => item.trim())
                            .filter(Boolean),
                        })
                      }
                    />
                  </label>
                </>
              )}
              <Input
                value={selected.blockedBy.join(', ')}
                onChange={(value) =>
                  updateTask({
                    blockedBy: value
                      .split(',')
                      .map((item) => item.trim())
                      .filter(Boolean),
                  })
                }
                disabled={editingDisabled}
                placeholder={t('productFactory.taskDraft.dependenciesField')}
              />
              <Input.TextArea
                value={selected.acceptanceCriteria.join('\n')}
                onChange={(value) => updateTask({ acceptanceCriteria: value.split('\n') })}
                disabled={editingDisabled}
                placeholder={t('productFactory.taskDraft.criteriaField')}
                autoSize={{ minRows: 3, maxRows: 7 }}
              />
              {!artifact.confirmed && (
                <Space>
                  <Button icon={<Up />} onClick={() => move(-1)} disabled={saving || selectedIndex === 0} />
                  <Button
                    icon={<Down />}
                    onClick={() => move(1)}
                    disabled={saving || selectedIndex === artifact.tasks.length - 1}
                  />
                  <Button icon={<Delete />} status='danger' onClick={remove} disabled={saving}>
                    {t('productFactory.taskDraft.delete')}
                  </Button>
                </Space>
              )}
            </Space>
          </div>
        )}
      </div>
      {!artifact.confirmed && (
        <Space className='mt-18px'>
          <Button icon={<Save />} onClick={onSave} loading={saving}>
            {t('productFactory.taskDraft.save')}
          </Button>
          <Button type='primary' onClick={onConfirm} loading={saving}>
            {t('productFactory.taskDraft.confirm')}
          </Button>
          {hasUnsavedChanges && (
            <span className='text-12px text-warning-6'>{t('productFactory.taskDraft.unsaved')}</span>
          )}
        </Space>
      )}
    </Card>
  );
};

export default TaskDraftReview;
