/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { useLayoutContext } from '@/renderer/hooks/context/LayoutContext';
import { Button, Dropdown, Menu, Tooltip } from '@arco-design/web-react';
import { MoreOne, Pushpin } from '@icon-park/react';
import classNames from 'classnames';
import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import styles from './Sider.module.css';

export type SiderMenuItem = {
  key: string;
  icon: React.ReactNode;
  label: string;
  danger?: boolean;
};

export type SiderItemProps = {
  icon: React.ReactNode;
  name: string;
  selected?: boolean;
  pinned?: boolean;
  menuItems?: SiderMenuItem[];
  onMenuAction?: (key: string) => void;
  onClick?: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
};

const SiderItem: React.FC<SiderItemProps> = ({
  icon,
  name,
  selected,
  pinned,
  menuItems,
  onMenuAction,
  onClick,
  onContextMenu,
}) => {
  const { t } = useTranslation();
  const [menuVisible, setMenuVisible] = useState(false);
  const layout = useLayoutContext();
  const isMobile = layout?.isMobile ?? false;

  const hasMenu = menuItems && menuItems.length > 0;

  return (
    <Tooltip
      content={name}
      disabled={!name}
      trigger='hover'
      popupVisible={name ? undefined : false}
      unmountOnExit
      popupHoverStay={false}
      position='top'
    >
      <div
        className={classNames(
          styles.itemRow,
          'h-34px rd-8px flex items-center gap-8px ps-10px pe-8px cursor-pointer relative overflow-hidden shrink-0 group min-w-0 transition-colors',
          {
            'hover:bg-fill-3': !selected,
            '!bg-fill-3': selected,
          }
        )}
        onContextMenu={onContextMenu}
      >
        <Button
          type='text'
          aria-label={name}
          aria-current={selected ? 'page' : undefined}
          className={styles.itemActivation}
          onClick={onClick}
        >
          {/* Leading icon — pushpin overlays this slot on hover when row is pinned */}
          <span className='size-22px flex items-center justify-center shrink-0 line-height-0 text-t-primary relative'>
            <span
              className={classNames('flex items-center justify-center', {
                'group-hover:opacity-0 transition-opacity': hasMenu && pinned,
              })}
            >
              {icon}
            </span>
            {hasMenu && pinned && (
              <span
                className='absolute inset-0 flex-center text-t-secondary pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity'
                style={{ lineHeight: 0 }}
              >
                <Pushpin theme='outline' size='14' />
              </span>
            )}
          </span>

          {/* Name with truncation — reserve room for the hover three-dot menu */}
          <span className='h-24px min-w-0 flex-1 overflow-hidden pe-12px'>
            <span className='overflow-hidden text-ellipsis block w-full text-14px font-[500] lh-24px whitespace-nowrap min-w-0 text-t-primary'>
              <span className='block overflow-hidden text-ellipsis whitespace-nowrap'>{name}</span>
            </span>
          </span>
        </Button>
        {/* Hover/active actions: three-dot menu */}
        {hasMenu && (
          <div
            className={classNames('absolute end-8px top-1/2 -translate-y-1/2 items-center justify-end', {
              flex: isMobile || menuVisible,
              'hidden group-hover:flex group-focus-within:flex': !isMobile && !menuVisible,
            })}
            onClick={(e) => e.stopPropagation()}
          >
            <Dropdown
              droplist={
                <Menu
                  onClickMenuItem={(key) => {
                    setMenuVisible(false);
                    onMenuAction?.(key);
                  }}
                >
                  {menuItems.map((item) => (
                    <Menu.Item key={item.key}>
                      <div
                        className={classNames('flex items-center gap-8px', {
                          'text-[rgb(var(--warning-6))]': item.danger,
                        })}
                      >
                        {item.icon}
                        <span>{item.label}</span>
                      </div>
                    </Menu.Item>
                  ))}
                </Menu>
              }
              trigger='click'
              position='br'
              popupVisible={menuVisible}
              onVisibleChange={setMenuVisible}
              getPopupContainer={() => document.body}
              unmountOnExit={false}
            >
              <Button
                type='text'
                aria-label={`${t('common.more')}: ${name}`}
                data-testid='sider-item-menu-trigger'
                className={classNames(
                  styles.itemMenuButton,
                  'flex-center cursor-pointer transition-colors text-t-secondary hover:text-t-primary size-20px rd-4px sider-action-btn',
                  {
                    flex: isMobile || menuVisible,
                    'hidden group-hover:flex group-focus-within:flex': !isMobile && !menuVisible,
                  }
                )}
                onClick={(e) => {
                  e.stopPropagation();
                  setMenuVisible(true);
                }}
              >
                <MoreOne theme='outline' size='14' fill='currentColor' className='block leading-none' />
              </Button>
            </Dropdown>
          </div>
        )}
      </div>
    </Tooltip>
  );
};

export default SiderItem;
