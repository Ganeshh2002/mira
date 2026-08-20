import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { CapabilityStatus } from '../bindings/CapabilityStatus';
import { StatusMark } from './StatusMark';

const full: CapabilityStatus = { state: 'full' };
const degraded: CapabilityStatus = {
  state: 'degraded',
  reason: 'The tray menu is the only way in on Linux',
  detail: 'Click events are never delivered to Linux tray icons.',
};
const unavailable: CapabilityStatus = {
  state: 'unavailable',
  reason: 'Wayland has no protocol for global shortcuts',
  fallback: 'mira --toggle',
};

describe('StatusMark', () => {
  it('never signals by colour alone', () => {
    // design-system §2 rule 1 and §10: every status carries a shape AND a text
    // partner, so it survives every form of colour blindness and every monitor.
    for (const status of [full, degraded, unavailable]) {
      const { unmount } = render(<StatusMark status={status} />);
      const mark = screen.getByRole('img');

      expect(mark).toHaveAttribute('aria-label');
      expect(mark.getAttribute('aria-label')).not.toHaveLength(0);
      expect(mark.textContent?.trim()).not.toHaveLength(0);
      unmount();
    }
  });

  it('gives each state a distinct mark', () => {
    const marks = [full, degraded, unavailable].map((status) => {
      const { container, unmount } = render(<StatusMark status={status} />);
      const text = container.textContent ?? '';
      unmount();
      return text;
    });

    expect(new Set(marks).size).toBe(3);
  });

  it('names the state in words a person reads', () => {
    render(<StatusMark status={unavailable} />);
    expect(screen.getByRole('img')).toHaveAccessibleName(/unavailable/i);
  });

  it('reads a degraded capability as working, not broken', () => {
    render(<StatusMark status={degraded} />);
    expect(screen.getByRole('img')).toHaveAccessibleName(/degraded/i);
  });
});
