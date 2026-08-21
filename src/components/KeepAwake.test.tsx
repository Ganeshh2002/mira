import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { KeepAwakeState } from '../bindings/KeepAwakeState';
import { KeepAwakePanel } from './KeepAwake';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const NOW = 1_800_000_000;

/** Serve a state, and whatever `set` produces. Remember the event handler. */
function backend(initial: KeepAwakeState, after?: KeepAwakeState) {
  let current = initial;
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    if (command === 'keep_awake_state') return Promise.resolve(current);
    if (command === 'keep_awake_set') {
      current = after ?? nextFor(args?.span as string);
      return Promise.resolve(current);
    }
    return Promise.resolve(undefined);
  });
}

function nextFor(span: string): KeepAwakeState {
  if (span === 'off') return { state: 'off' };
  return {
    state: 'on',
    span: span as 'thirtyMinutes' | 'oneHour' | 'untilTurnedOff',
    until: span === 'untilTurnedOff' ? null : NOW + 1800,
  };
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('keep awake', () => {
  it('is off until somebody turns it on', async () => {
    backend({ state: 'off' });
    renderApp(<KeepAwakePanel />);

    expect(await screen.findByText(/Off — your machine sleeps as usual/)).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Off' })).toBeChecked();
  });

  it('offers exactly four spans and no way to type a duration', async () => {
    backend({ state: 'off' });
    renderApp(<KeepAwakePanel />);

    const group = await screen.findByRole('radiogroup', { name: 'Keep Awake' });
    const choices = within(group).getAllByRole('radio');

    expect(choices.map((choice) => choice.getAttribute('aria-label'))).toEqual([
      'Off',
      '30 minutes',
      '1 hour',
      'Until turned off',
    ]);
    // A number field here would be a way to ask for a week. There is none.
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('spinbutton')).not.toBeInTheDocument();
  });

  it('sends the word, never a number of minutes', async () => {
    backend({ state: 'off' });
    renderApp(<KeepAwakePanel />);

    await userEvent.click(await screen.findByRole('radio', { name: '30 minutes' }));

    await waitFor(() => {
      const asked = invoke.mock.calls.find(([command]) => command === 'keep_awake_set');
      expect(asked?.[1]).toEqual({ span: 'thirtyMinutes' });
    });
  });

  it('shows which span is held, and when it ends', async () => {
    backend({ state: 'on', span: 'oneHour', until: NOW + 3600 });
    renderApp(<KeepAwakePanel />);

    expect(await screen.findByText('On · 1 hour')).toBeInTheDocument();
    expect(screen.getByText(/Ends at /)).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: '1 hour' })).toBeChecked();
  });

  it('does not promise an end for a span that has none', async () => {
    backend({ state: 'on', span: 'untilTurnedOff', until: null });
    renderApp(<KeepAwakePanel />);

    expect(await screen.findByText('On · until turned off')).toBeInTheDocument();
    expect(screen.queryByText(/Ends at /)).not.toBeInTheDocument();
  });

  it('turns off when Off is chosen', async () => {
    backend({ state: 'on', span: 'oneHour', until: NOW + 3600 });
    renderApp(<KeepAwakePanel />);

    await userEvent.click(await screen.findByRole('radio', { name: 'Off' }));

    await waitFor(() => {
      const asked = invoke.mock.calls.find(([command]) => command === 'keep_awake_set');
      expect(asked?.[1]).toEqual({ span: 'off' });
    });
    expect(await screen.findByText(/Off — your machine sleeps as usual/)).toBeInTheDocument();
  });

  it('says why it cannot work here, instead of showing a control that does nothing', async () => {
    backend({
      state: 'unavailable',
      reason: 'Mira does not yet hold a systemd-logind sleep inhibitor',
      fallback: "your desktop's power settings",
    });
    renderApp(<KeepAwakePanel />);

    expect(await screen.findByText('Not available on this machine')).toBeInTheDocument();
    expect(screen.getByText(/systemd-logind sleep inhibitor/)).toBeInTheDocument();
    expect(screen.getByText(/your desktop's power settings/)).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup')).not.toBeInTheDocument();
  });

  it('says what it does, and what it does not', async () => {
    // The claim that matters most for this feature. It is product copy, and it
    // is asserted so it cannot quietly become untrue (ADR-0014).
    backend({ state: 'off' });
    renderApp(<KeepAwakePanel />);

    const explanation = await screen.findByText(/never simulates typing or pointer movement/);
    expect(explanation).toHaveTextContent(/stops when you turn it off/);
    expect(explanation).toHaveTextContent(/when Mira quits/);
  });

  it('re-reads when the backend says a span ended on its own', async () => {
    backend({ state: 'on', span: 'thirtyMinutes', until: NOW + 1800 });
    renderApp(<KeepAwakePanel />);
    await screen.findByText('On · 30 minutes');

    // The subscription is the only thing that updates this panel: there is no
    // interval, and a guard test fails the build if one appears.
    expect(listen).toHaveBeenCalledWith('mira://keep-awake', expect.any(Function));
  });

  it('says so when a change did not take effect', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'keep_awake_state') return Promise.resolve({ state: 'off' });
      return Promise.reject({
        kind: 'unsupported',
        capability: 'keepAwake',
        reason: 'Mira has no way to ask this operating system to stay awake.',
      });
    });
    renderApp(<KeepAwakePanel />);

    await userEvent.click(await screen.findByRole('radio', { name: '1 hour' }));

    expect(await screen.findByText('That did not take effect')).toBeInTheDocument();
    expect(screen.getByText(/no way to ask this operating system/)).toBeInTheDocument();
  });
});
