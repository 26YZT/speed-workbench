/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { FactoryBuilding } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';
import SiderNavEntry from './SiderNavEntry';

type SiderProductFactoryEntryProps = {
  isActive: boolean;
  collapsed: boolean;
  isMobile?: boolean;
  siderTooltipProps: SiderTooltipProps;
  onClick: () => void;
};

const SiderProductFactoryEntry: React.FC<SiderProductFactoryEntryProps> = ({
  isActive,
  collapsed,
  isMobile,
  siderTooltipProps,
  onClick,
}) => {
  const { t } = useTranslation();
  const label = t('productFactory.sidebar');

  return (
    <SiderNavEntry
      label={label}
      icon={<FactoryBuilding theme='outline' size={collapsed ? 20 : 16} fill='currentColor' />}
      isActive={isActive}
      collapsed={collapsed}
      isMobile={isMobile}
      siderTooltipProps={siderTooltipProps}
      onClick={onClick}
      testId='product-factory-sider-entry'
    />
  );
};

export default SiderProductFactoryEntry;
