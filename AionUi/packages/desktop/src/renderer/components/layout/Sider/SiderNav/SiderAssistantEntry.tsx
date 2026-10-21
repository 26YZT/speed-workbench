/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Ghost } from '@icon-park/react';
import type { SiderTooltipProps } from '@renderer/utils/ui/siderTooltip';
import SiderNavEntry from './SiderNavEntry';

interface SiderAssistantEntryProps {
  isMobile: boolean;
  isActive: boolean;
  collapsed: boolean;
  siderTooltipProps: SiderTooltipProps;
  onClick: () => void;
}

const SiderAssistantEntry: React.FC<SiderAssistantEntryProps> = ({
  isMobile,
  isActive,
  collapsed,
  siderTooltipProps,
  onClick,
}) => {
  const { t } = useTranslation();

  return (
    <SiderNavEntry
      label={t('settings.assistants')}
      icon={<Ghost theme='outline' size={collapsed ? 20 : 16} fill='currentColor' />}
      isActive={isActive}
      collapsed={collapsed}
      isMobile={isMobile}
      siderTooltipProps={siderTooltipProps}
      onClick={onClick}
    />
  );
};

export default SiderAssistantEntry;
