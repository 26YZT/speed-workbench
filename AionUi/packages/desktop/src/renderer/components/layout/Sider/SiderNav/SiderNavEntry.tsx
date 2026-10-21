import React from 'react';
import { Button, Tooltip } from '@arco-design/web-react';
import classNames from 'classnames';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';
import styles from '../Sider.module.css';

type SiderNavEntryProps = {
  label: string;
  icon: React.ReactNode;
  isActive: boolean;
  collapsed: boolean;
  isMobile?: boolean;
  siderTooltipProps: SiderTooltipProps;
  onClick: () => void;
  testId?: string;
};

const SiderNavEntry: React.FC<SiderNavEntryProps> = ({
  label,
  icon,
  isActive,
  collapsed,
  isMobile = false,
  siderTooltipProps,
  onClick,
  testId,
}) => (
  <Tooltip {...siderTooltipProps} content={label} position='right'>
    <Button
      type='text'
      aria-label={label}
      aria-current={isActive ? 'page' : undefined}
      data-active={isActive}
      data-testid={testId}
      className={classNames(
        styles.navEntry,
        collapsed && styles.navEntryCollapsed,
        isMobile && 'sider-action-btn-mobile'
      )}
      onClick={onClick}
    >
      <span aria-hidden='true' className={styles.navEntryIcon}>
        {icon}
      </span>
      {!collapsed && <span className={styles.navEntryLabel}>{label}</span>}
    </Button>
  </Tooltip>
);

export default SiderNavEntry;
