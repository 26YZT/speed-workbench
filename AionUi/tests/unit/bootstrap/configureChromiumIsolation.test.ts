/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

type AppPath = 'userData' | 'logs';

describe('Chromium startup with an explicit E2E user-data sandbox', () => {
  let tempRoot: string;
  let home: string;
  let qaRoot: string;
  let legacyRegistry: string;
  let originalPaths: Record<AppPath, string>;
  let appPaths: Record<AppPath, string>;
  let originalRegistry: { content: string; inode: number };

  const registryState = () => ({ content: readFileSync(legacyRegistry, 'utf8'), inode: lstatSync(legacyRegistry).ino });

  const loadStartup = async (isPackaged = true) => {
    const app = {
      isPackaged,
      getPath: vi.fn((name: AppPath) => appPaths[name]),
      setPath: vi.fn((name: AppPath, value: string) => {
        appPaths[name] = value;
      }),
      setName: vi.fn(),
      commandLine: { appendSwitch: vi.fn() },
    };

    vi.doMock('electron', () => ({ app }));
    // The imported startup code can only see this fixture home, never the real home.
    const homedir = vi.fn(() => home);
    vi.doMock('os', () => ({ default: { homedir }, homedir }));
    vi.doMock('@/common/platform', () => ({ getDevAppName: () => 'Isolated Dev Workbench' }));
    vi.doMock('@process/utils/gpuRecovery', () => ({ applyGpuRecoveryFlags: vi.fn() }));

    await import('@process/utils/configureChromium');
    return app;
  };

  beforeEach(() => {
    vi.resetModules();
    tempRoot = mkdtempSync(path.join(tmpdir(), 'aionui-chromium-isolation-'));
    home = path.join(tempRoot, 'simulated-home');
    qaRoot = path.join(tempRoot, 'QA sandbox', 'userData');
    originalPaths = {
      userData: path.join(tempRoot, 'user-like Application Support', 'Speed Workbench'),
      logs: path.join(tempRoot, 'user-like Library', 'Logs', 'Speed Workbench'),
    };
    appPaths = { ...originalPaths };
    for (const folder of [home, ...Object.values(originalPaths)]) mkdirSync(folder, { recursive: true });
    legacyRegistry = path.join(home, '.aionui-cdp-registry.json');
    writeFileSync(legacyRegistry, JSON.stringify({ instances: [{ pid: 123, port: 9230, fixture: true }] }));
    writeFileSync(path.join(originalPaths.logs, 'sentinel.txt'), 'original logs');
    originalRegistry = registryState();
    vi.stubEnv('AIONUI_E2E_TEST', undefined);
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    for (const module of ['electron', 'os', '@/common/platform', '@process/utils/gpuRecovery']) vi.doUnmock(module);
    vi.resetModules();
    rmSync(tempRoot, { recursive: true, force: true });
  });

  it.each([true, false])(
    'isolates data and logs while preserving the home registry in packaged=%s startup',
    async (isPackaged) => {
      vi.stubEnv('AIONUI_E2E_TEST', '1');
      vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', qaRoot);

      const app = await loadStartup(isPackaged);

      expect({
        paths: appPaths,
        created: existsSync(path.join(qaRoot, 'logs')),
        firstRead: app.getPath.mock.results[0]?.value,
      }).toEqual({
        paths: { userData: qaRoot, logs: path.join(qaRoot, 'logs') },
        created: true,
        firstRead: qaRoot,
      });
      expect(registryState()).toEqual(originalRegistry);
      expect({
        appNames: app.setName.mock.calls,
        originalLogs: readFileSync(path.join(originalPaths.logs, 'sentinel.txt'), 'utf8'),
      }).toEqual({
        appNames: [],
        originalLogs: 'original logs',
      });
    }
  );

  it.each([undefined, '0', 'true'])('retains normal legacy cleanup without the exact E2E flag (%s)', async (flag) => {
    vi.stubEnv('AIONUI_E2E_TEST', flag);
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', qaRoot);

    const app = await loadStartup();

    expect(existsSync(legacyRegistry)).toBe(false);
    expect(appPaths).toEqual(originalPaths);
    expect(app.setPath).not.toHaveBeenCalled();
  });

  it.each([undefined, '', ' \t\n '])('retains normal legacy cleanup without a nonempty E2E root (%s)', async (root) => {
    vi.stubEnv('AIONUI_E2E_TEST', '1');
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', root);

    const app = await loadStartup();

    expect(existsSync(legacyRegistry)).toBe(false);
    expect(appPaths).toEqual(originalPaths);
    expect(app.setPath).not.toHaveBeenCalled();
  });
});
