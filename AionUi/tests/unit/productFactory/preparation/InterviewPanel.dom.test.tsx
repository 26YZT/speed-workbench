/**
 * @license
 * Copyright 2026 AionUi (aionui.com)
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import InterviewPanel from '@/renderer/pages/product-factory/components/Preparation/InterviewPanel';
import type { InterviewArtifact } from '@/renderer/pages/product-factory/types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const artifact: InterviewArtifact = {
  version: 1,
  questions: [{ id: 'audience', prompt: 'Who is this for?', answer: '', notSure: false }],
  summary: '',
  confirmed: false,
  updatedAt: 0,
};

describe('InterviewPanel', () => {
  it('does not allow confirmation until the summary is filled', () => {
    render(
      <InterviewPanel artifact={artifact} onChange={vi.fn()} onSave={vi.fn()} onConfirm={vi.fn()} saving={false} />
    );

    expect(screen.getByRole('button', { name: 'productFactory.interview.confirm' })).toBeDisabled();
  });

  it('emits an answer change and enables confirmation after a summary is entered', () => {
    const onChange = vi.fn();
    const Harness = () => {
      const [current, setCurrent] = React.useState(artifact);
      return (
        <InterviewPanel
          artifact={current}
          onChange={(next) => {
            onChange(next);
            setCurrent(next);
          }}
          onSave={vi.fn()}
          onConfirm={vi.fn()}
          saving={false}
        />
      );
    };
    render(<Harness />);

    fireEvent.change(screen.getByTestId('product-factory-interview-answer-audience'), {
      target: { value: 'Product teams' },
    });
    fireEvent.change(screen.getByTestId('product-factory-interview-summary'), {
      target: { value: 'A research brief for product teams' },
    });

    expect(onChange).toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'productFactory.interview.confirm' })).not.toBeDisabled();
  });
});
