/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Button, Tooltip } from '@arco-design/web-react';
import { FactoryBuilding } from '@icon-park/react';
import { useTranslation } from 'react-i18next';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';

type SiderProductFactoryEntryProps = {
  isActive: boolean;
  collapsed: boolean;
  siderTooltipProps: SiderTooltipProps;
  onClick: () => void;
};

const SiderProductFactoryEntry: React.FC<SiderProductFactoryEntryProps> = ({
  isActive,
  collapsed,
  siderTooltipProps,
  onClick,
}) => {
  const { t } = useTranslation();
  const label = t('productFactory.sidebar');

  return (
    <Tooltip {...siderTooltipProps} content={label} position='right'>
      <Button
        type={isActive ? 'secondary' : 'text'}
        aria-label={label}
        data-testid='product-factory-sider-entry'
        className='w-full min-h-34px text-t-primary'
        icon={<FactoryBuilding theme='outline' size={collapsed ? 20 : 17} fill='currentColor' />}
        onClick={onClick}
      >
        {!collapsed && label}
      </Button>
    </Tooltip>
  );
};

export default SiderProductFactoryEntry;
