/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Tooltip } from '@arco-design/web-react';
import { ListCheckbox, Plus } from '@icon-park/react';
import classNames from 'classnames';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';
import styles from '../Sider.module.css';

interface SiderToolbarProps {
  isMobile: boolean;
  isBatchMode: boolean;
  collapsed: boolean;
  siderTooltipProps: SiderTooltipProps;
  onNewChat: () => void;
  onToggleBatchMode: () => void;
}

const SiderToolbar: React.FC<SiderToolbarProps> = ({
  isMobile,
  isBatchMode,
  collapsed,
  siderTooltipProps,
  onNewChat,
  onToggleBatchMode,
}) => {
  const { t } = useTranslation();

  const label = t('conversation.welcome.newConversation');
  const batchLabel = isBatchMode ? t('conversation.history.batchModeExit') : t('conversation.history.batchManage');
  return (
    <div className='shrink-0 flex items-center gap-8px'>
      <Tooltip {...siderTooltipProps} content={label} position='right'>
        <Button
          type='text'
          aria-label={label}
          onClick={onNewChat}
          className={classNames(
            styles.navEntry,
            styles.toolbarAction,
            styles.newChatTrigger,
            'group',
            collapsed && styles.navEntryCollapsed,
            isMobile && 'sider-action-btn-mobile'
          )}
        >
          <span
            aria-hidden='true'
            className={classNames(
              'size-22px flex items-center justify-center shrink-0',
              !collapsed &&
                'rd-6px bg-aou-2 border border-solid border-[var(--color-border-2)] group-hover:bg-fill-3 group-hover:border-transparent'
            )}
          >
            <Plus
              theme='outline'
              size={collapsed ? 16 : 14}
              fill='currentColor'
              className={classNames('block leading-none', styles.newChatIcon)}
            />
          </span>
          {!collapsed && <span className={styles.navEntryLabel}>{label}</span>}
        </Button>
      </Tooltip>
      {!collapsed && (
        <Tooltip {...siderTooltipProps} content={batchLabel} position='right'>
          <Button
            type='text'
            aria-label={batchLabel}
            aria-pressed={isBatchMode}
            onClick={onToggleBatchMode}
            className={classNames(
              styles.iconAction,
              isMobile && 'sider-action-icon-btn-mobile',
              isBatchMode && 'bg-fill-3 !text-primary'
            )}
          >
            <ListCheckbox theme='outline' size={14} fill='currentColor' aria-hidden='true' />
          </Button>
        </Tooltip>
      )}
    </div>
  );
};

export default SiderToolbar;
