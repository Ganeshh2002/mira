import { screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { FoundationStatus } from './bindings/FoundationStatus';
import type { KeepAwakeState } from './bindings/KeepAwakeState';
import { App } from './App';
import { renderApp } from './test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const status: FoundationStatus = {
  appName: 'Mira',
  appVersion: '0.1.0',
  platform: 'Linux',
  session: 'Wayland',
  databasePath: '/home/dev/.local/share/dev.aviora.mira/mira.db',
  schemaVersion: 1,
  projectCount: 0,
  workspaceCount: 0,
  shortcutChord: 'Ctrl+Alt+Space',
  shortcutRegistered: false,
  surface: 'opaque',
  capabilities: [
    {
      capability: 'globalShortcut',
      label: 'Global shortcut',
      status: {
        state: 'unavailable',
        reason: 'Wayland has no protocol for global shortcuts',
        fallback: 'mira --toggle',
      },
    },
    {
      capability: 'trayIcon',
      label: 'Tray icon',
      status: {
        state: 'degraded',
        reason: 'The tray menu is the only way in on Linux',
        detail: 'Click events are never delivered to Linux tray icons.',
      },
    },
    {
      capability: 'portEnumeration',
      label: 'Port enumeration',
      status: { state: 'full' },
    },
  ],
};

/** Settings asks two questions. Answer each with the command that was called. */
function backend(awake: KeepAwakeState) {
  invoke.mockImplementation((command: string) => {
    if (command === 'keep_awake_state') return Promise.resolve(awake);
    if (command === 'keep_awake_set') return Promise.resolve(awake);
    return Promise.resolve(status);
  });
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
  backend({ state: 'off' });
});

describe('the settings window', () => {
  it('lists every capability with its resolved state', async () => {
    renderApp(<App surface="settings" />);

    expect(await screen.findByText('Global shortcut')).toBeInTheDocument();
    expect(screen.getByText('Tray icon')).toBeInTheDocument();
    expect(screen.getByText('Port enumeration')).toBeInTheDocument();
  });

  it('states the reason an unavailable capability is off', async () => {
    renderApp(<App surface="settings" />);

    expect(
      await screen.findByText(/Wayland has no protocol for global shortcuts/),
    ).toBeInTheDocument();
  });

  it('shows the fallback so a Wayland user is not left stuck', async () => {
    renderApp(<App surface="settings" />);

    expect(await screen.findByText(/mira --toggle/)).toBeInTheDocument();
  });

  it('shows where the database lives so the user can delete it', async () => {
    renderApp(<App surface="settings" />);

    expect(await screen.findByText(status.databasePath)).toBeInTheDocument();
  });
});
