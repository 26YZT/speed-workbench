/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Navigation history stack for the in-app back/forward buttons.
 *
 * Behaves like a browser history: every time the route pathname changes we
 * push an entry; calling back()/forward() moves the cursor; when a new
 * navigation happens while the cursor is in the middle of the stack, entries
 * after the cursor are discarded (just like opening a new page after going
 * back in a browser).
 *
 * This captures route-level navigations only (e.g. switching conversations,
 * opening settings). Intra-page interactions like scrolling or sending a
 * message do not change the pathname and are correctly ignored.
 */

import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import {
  useLocation,
  useNavigate,
  useNavigationType,
  NavigationType,
  UNSAFE_NavigationContext,
} from 'react-router-dom';
import { Modal } from '@arco-design/web-react';

type PageLeaveGuard = { blocked: () => boolean; confirm: () => Promise<boolean> };
let pageLeaveGuard: PageLeaveGuard | undefined;
let bypassGuard = false;
let pendingNavigation = false;

const requestPageNavigation = (action: () => void): void => {
  if (bypassGuard || !pageLeaveGuard?.blocked()) {
    action();
    return;
  }
  if (pendingNavigation) return;
  pendingNavigation = true;
  void pageLeaveGuard.confirm().then((allowed) => {
    pendingNavigation = false;
    if (!allowed) return;
    bypassGuard = true;
    try {
      action();
    } finally {
      bypassGuard = false;
    }
  });
};

/** Protect in-app navigation; persisted recovery also covers browser/native exits. */
export function usePageLeaveGuard(
  blocked: boolean,
  labels: { title: string; content: string; leave: string; stay: string },
  preventUnload = blocked
): void {
  const context = useContext(UNSAFE_NavigationContext);
  const current = useRef({ blocked, labels, preventUnload });
  current.current = { blocked, labels, preventUnload };
  useEffect(() => {
    if (!context) return;
    const navigator = context.navigator;
    const original = { push: navigator.push, replace: navigator.replace, go: navigator.go };
    let pending: Promise<boolean> | undefined;
    let closeDialog: (() => void) | undefined;
    const guard: PageLeaveGuard = {
      blocked: () => current.current.blocked,
      confirm: () => {
        if (pending) return pending;
        pending = new Promise<boolean>((resolve) => {
          const { title, content, leave, stay } = current.current.labels;
          const finish = (allowed: boolean) => {
            pending = undefined;
            resolve(allowed);
          };
          const controller = Modal.confirm({
            title,
            content,
            okText: leave,
            cancelText: stay,
            onOk: () => finish(true),
            onCancel: () => finish(false),
          });
          closeDialog = () => {
            controller.close();
            finish(false);
          };
        });
        return pending;
      },
    };
    pageLeaveGuard = guard;
    const push: typeof navigator.push = (...args) => requestPageNavigation(() => original.push.apply(navigator, args));
    const replace: typeof navigator.replace = (...args) =>
      requestPageNavigation(() => original.replace.apply(navigator, args));
    const go: typeof navigator.go = (...args) => requestPageNavigation(() => original.go.apply(navigator, args));
    navigator.push = push;
    navigator.replace = replace;
    navigator.go = go;
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (current.current.preventUnload) {
        event.preventDefault();
        event.returnValue = '';
      }
    };
    window.addEventListener('beforeunload', beforeUnload);
    return () => {
      if (navigator.push === push) navigator.push = original.push;
      if (navigator.replace === replace) navigator.replace = original.replace;
      if (navigator.go === go) navigator.go = original.go;
      if (pageLeaveGuard === guard) pageLeaveGuard = undefined;
      closeDialog?.();
      window.removeEventListener('beforeunload', beforeUnload);
    };
  }, [context]);
}

const MAX_HISTORY = 50;

type HistoryEntry = {
  path: string; // full path including search + hash
};

type NavigationHistoryContextValue = {
  canBack: boolean;
  canForward: boolean;
  back: () => void;
  forward: () => void;
};

const NavigationHistoryContext = createContext<NavigationHistoryContextValue | null>(null);

const buildPath = (location: { pathname: string; search: string; hash: string }) =>
  `${location.pathname}${location.search}${location.hash}`;

export const NavigationHistoryProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
  const location = useLocation();
  const navigate = useNavigate();
  const navigationType = useNavigationType();

  const [stack, setStack] = useState<HistoryEntry[]>(() => [{ path: buildPath(location) }]);
  const [cursor, setCursor] = useState(0);

  // When back()/forward() triggers navigate(), the location listener below
  // would otherwise push a duplicate entry. This ref tells the listener to
  // skip the next location change.
  const skipNextRef = useRef(false);

  useEffect(() => {
    if (skipNextRef.current) {
      skipNextRef.current = false;
      return;
    }
    const path = buildPath(location);
    setStack((prevStack) => {
      const prevEntry = prevStack[cursor];
      if (prevEntry && prevEntry.path === path) {
        // Same path as current cursor — no-op (initial render, or a redundant push).
        return prevStack;
      }
      // navigate(..., { replace: true }) should overwrite the current cursor
      // entry rather than push a new one — otherwise replace navigations grow
      // the in-app history stack and incorrectly enable the back button (e.g.
      // when ConversationIndex redirects from a 404'd conversation to '/').
      if (navigationType === NavigationType.Replace) {
        const next = prevStack.slice();
        next[cursor] = { path };
        return next;
      }
      // POP (browser/native back-forward): we already mutate cursor + stack
      // inside back()/forward(); skipNextRef should have caught those. Any
      // POP that slips through here came from outside our buttons (e.g.
      // hardware back), so treat it like a push for consistency.
      // Discard any forward entries past the cursor, then append.
      const truncated = prevStack.slice(0, cursor + 1);
      truncated.push({ path });
      // Enforce max length by dropping from the oldest side.
      if (truncated.length > MAX_HISTORY) {
        const overflow = truncated.length - MAX_HISTORY;
        const trimmed = truncated.slice(overflow);
        setCursor(trimmed.length - 1);
        return trimmed;
      }
      setCursor(truncated.length - 1);
      return truncated;
    });
    // We intentionally only depend on pathname/search/hash — not `cursor` —
    // because `cursor` is kept consistent inside the setStack updater above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname, location.search, location.hash, navigationType]);

  const back = useCallback(() => {
    const next = cursor - 1;
    if (next < 0) return;
    const target = stack[next];
    if (!target) return;
    requestPageNavigation(() => {
      skipNextRef.current = true;
      setCursor(next);
      // Use { replace: true } so traversing our own stack doesn't grow
      // React Router's history unboundedly — this is meant to emulate
      // browser back/forward, which never creates new history entries.
      void navigate(target.path, { replace: true });
    });
  }, [cursor, stack, navigate]);

  const forward = useCallback(() => {
    const next = cursor + 1;
    if (next >= stack.length) return;
    const target = stack[next];
    if (!target) return;
    requestPageNavigation(() => {
      skipNextRef.current = true;
      setCursor(next);
      void navigate(target.path, { replace: true });
    });
  }, [cursor, stack, navigate]);

  const value = useMemo<NavigationHistoryContextValue>(
    () => ({
      canBack: cursor > 0,
      canForward: cursor < stack.length - 1,
      back,
      forward,
    }),
    [cursor, stack.length, back, forward]
  );

  return <NavigationHistoryContext.Provider value={value}>{children}</NavigationHistoryContext.Provider>;
};

export const useNavigationHistory = (): NavigationHistoryContextValue | null => {
  return useContext(NavigationHistoryContext);
};
