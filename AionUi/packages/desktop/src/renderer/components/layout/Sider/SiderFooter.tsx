/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Tooltip } from '@arco-design/web-react';
import { ArrowCircleLeft, CloseOne, Moon, SettingTwo, SunOne } from '@icon-park/react';
import classNames from 'classnames';
import styles from './Sider.module.css';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';

interface SiderFooterProps {
  isMobile: boolean;
  isSettings: boolean;
  collapsed?: boolean;
  theme: string;
  siderTooltipProps: SiderTooltipProps;
  onSettingsClick: () => void;
  onThemeToggle: () => void;
  showLogout?: boolean;
  onLogoutClick?: () => void;
}

const SiderFooter: React.FC<SiderFooterProps> = ({
  isMobile,
  isSettings,
  collapsed = false,
  theme,
  siderTooltipProps,
  onSettingsClick,
  onThemeToggle,
  showLogout = false,
  onLogoutClick,
}) => {
  const { t } = useTranslation();

  const settingsIcon = isSettings ? (
    <ArrowCircleLeft
      theme='outline'
      size='16'
      fill='currentColor'
      className='block leading-none'
      style={{ lineHeight: 0 }}
    />
  ) : (
    <SettingTwo
      theme='outline'
      size='16'
      fill='currentColor'
      className='block leading-none'
      style={{ lineHeight: 0 }}
    />
  );
  const showThemeToggle = isSettings && !collapsed;
  const themeTooltip = theme === 'dark' ? t('settings.lightMode') : t('settings.darkMode');

  const label = isSettings ? t('common.back') : t('common.settings');
  return (
    <div className='shrink-0 sider-footer mt-auto pt-8px pb-8px border-t border-solid border-[var(--color-border-2)] border-s-0 border-e-0 border-b-0'>
      <div className={classNames('flex', collapsed ? 'flex-col gap-2px' : 'items-center gap-2px')}>
        <Tooltip {...siderTooltipProps} content={label} position='right'>
          <Button
            type='text'
            aria-label={label}
            onClick={onSettingsClick}
            className={classNames(
              styles.navEntry,
              styles.toolbarAction,
              collapsed && styles.navEntryCollapsed,
              isMobile && 'sider-action-btn-mobile'
            )}
          >
            <span aria-hidden='true' className={styles.navEntryIcon}>
              {settingsIcon}
            </span>
            {!collapsed && <span className={styles.navEntryLabel}>{label}</span>}
          </Button>
        </Tooltip>
        {showLogout && onLogoutClick && (
          <Tooltip {...siderTooltipProps} content={t('settings.googleLogout')} position='right'>
            <Button
              type='text'
              aria-label={t('settings.googleLogout')}
              onClick={onLogoutClick}
              className={classNames(
                styles.navEntry,
                styles.toolbarAction,
                collapsed && styles.navEntryCollapsed,
                isMobile && 'sider-action-btn-mobile'
              )}
            >
              <span aria-hidden='true' className={styles.navEntryIcon}>
                <CloseOne theme='outline' size={16} fill='currentColor' />
              </span>
              {!collapsed && <span className={styles.navEntryLabel}>{t('settings.googleLogout')}</span>}
            </Button>
          </Tooltip>
        )}
        {showThemeToggle && (
          <Tooltip {...siderTooltipProps} content={themeTooltip} position='right'>
            <Button
              type='text'
              onClick={onThemeToggle}
              data-testid='theme-toggle'
              aria-label={themeTooltip}
              className={classNames(styles.iconAction, styles.footerIcon, isMobile && 'sider-footer-btn-mobile')}
            >
              {theme === 'dark' ? (
                <SunOne theme='outline' size={18} fill='currentColor' />
              ) : (
                <Moon theme='outline' size={18} fill='currentColor' />
              )}
            </Button>
          </Tooltip>
        )}
      </div>
    </div>
  );
};

export default SiderFooter;
