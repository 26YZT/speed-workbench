/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => (key === 'productFactory.sidebar' ? 'Product Factory' : key) }),
}));

import SiderProductFactoryEntry from '@/renderer/components/layout/Sider/SiderNav/SiderProductFactoryEntry';

describe('SiderProductFactoryEntry', () => {
  it('exposes an accessible label and delegates navigation when clicked', () => {
    const onClick = vi.fn();

    render(<SiderProductFactoryEntry isActive={false} collapsed={false} siderTooltipProps={{}} onClick={onClick} />);

    fireEvent.click(screen.getByRole('button', { name: 'Product Factory' }));

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('keeps the accessible label when the sidebar is collapsed', () => {
    render(<SiderProductFactoryEntry isActive collapsed siderTooltipProps={{}} onClick={() => undefined} />);

    expect(screen.getByRole('button', { name: 'Product Factory' })).toBeInTheDocument();
  });
});
