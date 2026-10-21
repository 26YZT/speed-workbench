import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

const state = vi.hoisted(() => ({
  pathname: '/product-factory',
  mobile: false,
  navigate: vi.fn(),
  closePreview: vi.fn(),
}));

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock('react-router-dom', () => ({
  useLocation: () => ({ pathname: state.pathname, search: '', hash: '' }),
  useNavigate: () => state.navigate,
}));
vi.mock('@renderer/pages/conversation/Preview/context/PreviewContext', () => ({
  usePreviewContext: () => ({ closePreview: state.closePreview, clearPreviewForScope: vi.fn() }),
}));
vi.mock('@renderer/hooks/context/AuthContext', () => ({
  useAuth: () => ({ logout: vi.fn(), status: 'authenticated' }),
}));
vi.mock('@renderer/hooks/context/LayoutContext', () => ({
  useLayoutContext: () => ({ isMobile: state.mobile }),
}));
vi.mock('@renderer/hooks/context/ThemeContext', () => ({
  useThemeContext: () => ({ theme: 'light', setTheme: vi.fn() }),
}));
vi.mock('@renderer/utils/ui/siderTooltip', () => ({
  cleanupSiderTooltips: vi.fn(),
  getSiderTooltipProps: (enabled: boolean) => ({ disabled: !enabled }),
}));
vi.mock('@renderer/pages/conversation/GroupedHistory', () => ({ default: () => null }));
vi.mock('@renderer/pages/settings/components/SettingsSider', () => ({ default: () => null }));
vi.mock('@renderer/components/layout/Sider/TeamSiderSection', () => ({ default: () => null }));
vi.mock('@renderer/components/layout/Sider/SiderNav/SiderSearchEntry', () => ({ default: () => null }));

import Sider from '@renderer/components/layout/Sider';

const labels = ['productFactory.sidebar', 'common.modelBench.title', 'settings.assistants', 'cron.scheduledTasks'];

describe('workspace primary navigation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    state.pathname = '/product-factory';
    state.mobile = false;
  });

  it('exposes four peer button destinations in product-workflow order', () => {
    render(<Sider />);
    const navigation = screen.getByRole('navigation');
    expect(
      within(navigation)
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label'))
    ).toEqual(labels);
  });

  it.each([
    ['/product-factory/run-1', 'productFactory.sidebar'],
    ['/model-bench', 'common.modelBench.title'],
    ['/assistants', 'settings.assistants'],
    ['/scheduled', 'cron.scheduledTasks'],
  ])('marks only the destination for %s as the current page', (pathname, label) => {
    state.pathname = pathname;
    render(<Sider />);
    const current = screen.getByRole('navigation').querySelectorAll('[aria-current="page"]');
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveAccessibleName(label);
  });

  it('does not mark a destination for an unrelated route with a similar prefix', () => {
    state.pathname = '/product-factory-other';
    render(<Sider />);
    expect(screen.getByRole('navigation').querySelector('[aria-current="page"]')).toBeNull();
  });

  it('keeps every destination accessible when the sidebar is collapsed', () => {
    render(<Sider collapsed />);
    const navigation = screen.getByRole('navigation');
    labels.forEach((label) => expect(within(navigation).getByRole('button', { name: label })).toBeInTheDocument());
    expect(navigation.textContent).toBe('');
  });

  it.each([
    ['productFactory.sidebar', '/product-factory'],
    ['common.modelBench.title', '/model-bench'],
    ['settings.assistants', '/assistants'],
    ['cron.scheduledTasks', '/scheduled'],
  ])('closes the mobile sidebar and clears the preview when opening %s', (label, pathname) => {
    state.mobile = true;
    const onSessionClick = vi.fn();
    render(<Sider onSessionClick={onSessionClick} />);
    fireEvent.click(within(screen.getByRole('navigation')).getByRole('button', { name: label }));
    expect(state.navigate).toHaveBeenCalledWith(pathname);
    expect(state.closePreview).toHaveBeenCalledOnce();
    expect(onSessionClick).toHaveBeenCalledOnce();
  });
});
