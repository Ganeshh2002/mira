import { screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { FoundationStatus } from './bindings/FoundationStatus';
import { App } from './App';
import { renderApp } from './test/render';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

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

beforeEach(() => {
  invoke.mockReset();
});

describe('the main window', () => {
  it('identifies itself as a foundation build, not a finished product', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="main" />);

    expect(await screen.findByText('Mira')).toBeInTheDocument();
    expect(screen.getByText(/foundation build/i)).toBeInTheDocument();
  });

  it('reports the platform it actually resolved at runtime', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="main" />);

    expect(await screen.findByText('Linux')).toBeInTheDocument();
    expect(screen.getByText('Ready')).toBeInTheDocument();
  });

  it('states the reason when a command fails, and does not claim to be ready', async () => {
    invoke.mockRejectedValue({ kind: 'external', source: 'SQLite', detail: 'disk I/O error' });
    renderApp(<App surface="main" />);

    await waitFor(() => {
      expect(screen.getByText(/disk I\/O error/)).toBeInTheDocument();
    });
    expect(screen.queryByText('Ready')).not.toBeInTheDocument();
  });

  it('does not build the dashboard', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="main" />);
    await screen.findByText('Ready');

    // Slice 0 ships a shell verification screen. Anything below belongs to a later
    // slice, and its appearance here would be scope leaking in.
    for (const absent of [/projects/i, /ports/i, /git/i, /branch/i, /shelf/i]) {
      expect(screen.queryByText(absent)).not.toBeInTheDocument();
    }
  });
});

describe('the settings window', () => {
  it('lists every capability with its resolved state', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="settings" />);

    expect(await screen.findByText('Global shortcut')).toBeInTheDocument();
    expect(screen.getByText('Tray icon')).toBeInTheDocument();
    expect(screen.getByText('Port enumeration')).toBeInTheDocument();
  });

  it('states the reason an unavailable capability is off', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="settings" />);

    expect(
      await screen.findByText(/Wayland has no protocol for global shortcuts/),
    ).toBeInTheDocument();
  });

  it('shows the fallback so a Wayland user is not left stuck', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="settings" />);

    expect(await screen.findByText(/mira --toggle/)).toBeInTheDocument();
  });

  it('shows where the database lives so the user can delete it', async () => {
    invoke.mockResolvedValue(status);
    renderApp(<App surface="settings" />);

    expect(await screen.findByText(status.databasePath)).toBeInTheDocument();
  });
});
