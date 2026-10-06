/**
 * @license
 * Copyright 2025 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  readdirSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const platformPaths = vi.hoisted(() => ({
  getHomeDir: vi.fn<() => string>(),
  getDataDir: vi.fn<() => string>(),
  needsCliSafeSymlinks: vi.fn(() => true),
  isPackaged: vi.fn(() => true),
}));

vi.mock('@/common/platform', async (importOriginal) => {
  const original = await importOriginal<typeof import('@/common/platform')>();
  return { ...original, getPlatformServices: () => ({ paths: platformPaths }) };
});

import { getConfigPath, getDataPath } from '@process/utils/utils';

// Windows directory symlinks can require privileges that ordinary CI does not have.
describe.skipIf(process.platform === 'win32')('CLI-safe paths with an explicit E2E user-data sandbox', () => {
  let tempRoot: string;
  let home: string;
  let qaRoot: string;
  let existingData: string;
  let existingConfig: string;

  const aliases = () => ['.aionui', '.aionui-config'].map((name) => path.join(home, name));
  const readAliasState = () => aliases().map((alias) => ({ target: readlinkSync(alias), inode: lstatSync(alias).ino }));
  const resolvePaths = () => ({ data: getDataPath(), config: getConfigPath() });
  const expectedQaPaths = () => ({ data: path.join(qaRoot, 'aionui'), config: path.join(qaRoot, 'config') });
  const originalSentinels = () => [
    readFileSync(path.join(existingData, 'sentinel.txt'), 'utf8'),
    readFileSync(path.join(existingConfig, 'sentinel.txt'), 'utf8'),
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    // Every filesystem path belongs to this fixture; never replace process.env.HOME.
    tempRoot = mkdtempSync(path.join(os.tmpdir(), 'aionui-cli-path-isolation-'));
    home = path.join(tempRoot, 'simulated-home');
    qaRoot = path.join(tempRoot, 'QA sandbox', 'userData');
    existingData = path.join(tempRoot, 'user-like Application Support', 'Speed Workbench', 'aionui');
    existingConfig = path.join(tempRoot, 'user-like Application Support', 'Speed Workbench', 'config');
    for (const folder of [home, existingData, existingConfig]) mkdirSync(folder, { recursive: true });
    writeFileSync(path.join(existingData, 'sentinel.txt'), 'original data');
    writeFileSync(path.join(existingConfig, 'sentinel.txt'), 'original config');
    symlinkSync(existingData, path.join(home, '.aionui'), 'dir');
    symlinkSync(existingConfig, path.join(home, '.aionui-config'), 'dir');
    platformPaths.getHomeDir.mockReturnValue(home);
    platformPaths.getDataDir.mockReturnValue(qaRoot);
    platformPaths.needsCliSafeSymlinks.mockReturnValue(true);
    platformPaths.isPackaged.mockReturnValue(true);
    vi.stubEnv('AIONUI_E2E_TEST', undefined);
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    rmSync(tempRoot, { recursive: true, force: true });
  });

  it('returns QA paths without replacing either existing macOS home alias or its target data', () => {
    vi.stubEnv('AIONUI_E2E_TEST', '1');
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', qaRoot);
    const originalAliases = readAliasState();

    expect(resolvePaths()).toEqual(expectedQaPaths());
    expect(readAliasState()).toEqual(originalAliases);
    expect(originalSentinels()).toEqual(['original data', 'original config']);
  });

  it('does not create global home aliases when the QA sandbox has none', () => {
    for (const alias of aliases()) unlinkSync(alias);
    vi.stubEnv('AIONUI_E2E_TEST', '1');
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', qaRoot);

    expect(resolvePaths()).toEqual(expectedQaPaths());
    expect(readdirSync(home)).toEqual([]);
  });

  it.each([undefined, '0', 'true'])('keeps normal macOS alias repair when the E2E flag is %s', (flag) => {
    vi.stubEnv('AIONUI_E2E_TEST', flag);
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', qaRoot);

    expect(resolvePaths()).toEqual({ data: aliases()[0], config: aliases()[1] });
    expect(aliases().map((alias) => readlinkSync(alias))).toEqual([expectedQaPaths().data, expectedQaPaths().config]);
    expect(originalSentinels()).toEqual(['original data', 'original config']);
  });

  it.each([undefined, '', ' \t\n '])('keeps normal macOS alias repair when the sandbox root is %s', (root) => {
    vi.stubEnv('AIONUI_E2E_TEST', '1');
    vi.stubEnv('AIONUI_E2E_USER_DATA_DIR', root);

    expect(resolvePaths()).toEqual({ data: aliases()[0], config: aliases()[1] });
    expect(aliases().map((alias) => readlinkSync(alias))).toEqual([expectedQaPaths().data, expectedQaPaths().config]);
    expect(originalSentinels()).toEqual(['original data', 'original config']);
  });
});
