import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter, useLocation, useNavigate } from 'react-router-dom';
import { Modal } from '@arco-design/web-react';
import {
  NavigationHistoryProvider,
  useNavigationHistory,
  usePageLeaveGuard,
} from '@/renderer/hooks/context/NavigationHistoryContext';

let confirmOptions: Parameters<typeof Modal.confirm>[0];
function Editor({ preventUnload = true }: { preventUnload?: boolean }) {
  const location = useLocation();
  const navigate = useNavigate();
  const history = useNavigationHistory();
  usePageLeaveGuard(
    location.pathname === '/edit',
    {
      title: 'Leave?',
      content: 'Draft kept locally',
      leave: 'Leave',
      stay: 'Stay',
    },
    preventUnload
  );
  return (
    <>
      <span data-testid='path'>{location.pathname}</span>
      <button onClick={() => navigate('/edit')}>Edit</button>
      <button onClick={() => navigate('/other')}>Other</button>
      <button onClick={() => navigate('/third')}>Third</button>
      <button disabled={!history?.canBack} onClick={history?.back}>
        Back
      </button>
    </>
  );
}
const mount = (path: string) =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <NavigationHistoryProvider>
        <Editor />
      </NavigationHistoryProvider>
    </MemoryRouter>
  );
const mockConfirm = () =>
  vi.spyOn(Modal, 'confirm').mockImplementation((options) => {
    confirmOptions = options;
    return { close: vi.fn(), update: vi.fn() };
  });
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('draft-aware navigation', () => {
  it('allows closing when the draft is already durable while retaining in-app leave protection', () => {
    mockConfirm();
    render(
      <MemoryRouter initialEntries={['/edit']}>
        <NavigationHistoryProvider>
          <Editor preventUnload={false} />
        </NavigationHistoryProvider>
      </MemoryRouter>
    );
    const event = new Event('beforeunload', { cancelable: true });
    expect(window.dispatchEvent(event)).toBe(true);
    fireEvent.click(screen.getByText('Other'));
    expect(Modal.confirm).toHaveBeenCalledOnce();
  });
  it('keeps the current page when the user cancels and leaves only after confirmation', async () => {
    const confirm = mockConfirm();
    mount('/edit');
    fireEvent.click(screen.getByText('Other'));
    expect(confirm).toHaveBeenCalledOnce();
    expect(screen.getByTestId('path')).toHaveTextContent('/edit');
    await act(async () => {
      await confirmOptions.onCancel?.();
    });
    expect(screen.getByTestId('path')).toHaveTextContent('/edit');
    fireEvent.click(screen.getByText('Other'));
    await act(async () => {
      await confirmOptions.onOk?.();
    });
    expect(screen.getByTestId('path')).toHaveTextContent('/other');
  });
  it('does not advance the in-app history cursor when a guarded back action is canceled', async () => {
    mockConfirm();
    mount('/home');
    fireEvent.click(screen.getByText('Edit'));
    await waitFor(() => expect(screen.getByText('Back')).not.toBeDisabled());
    fireEvent.click(screen.getByText('Back'));
    await act(async () => {
      await confirmOptions.onCancel?.();
    });
    expect(screen.getByTestId('path')).toHaveTextContent('/edit');
    fireEvent.click(screen.getByText('Back'));
    await act(async () => {
      await confirmOptions.onOk?.();
    });
    expect(screen.getByTestId('path')).toHaveTextContent('/home');
  });
  it('keeps the first requested destination while a confirmation is already open', async () => {
    const confirm = mockConfirm();
    mount('/edit');
    fireEvent.click(screen.getByText('Other'));
    fireEvent.click(screen.getByText('Third'));
    expect(confirm).toHaveBeenCalledOnce();
    await act(async () => {
      await confirmOptions.onOk?.();
    });
    expect(screen.getByTestId('path')).toHaveTextContent('/other');
  });
});
