import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppReport } from '../bindings/AppReport';
import type { ChosenApp } from '../bindings/ChosenApp';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { Workspace } from '../bindings/Workspace';
import { App } from '../App';
import { renderApp } from '../test/render';

/**
 * Opening a workspace's context in a real application.
 *
 * The property every test here circles: **the interface asks for a kind.** It
 * never sends a path, a program, a command line or an address, and there is no
 * argument through which it could. What actually opens is decided in Rust, so
 * these tests assert the *shape of the request* as carefully as they assert what
 * appears on screen (slice brief §11).
 */

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const NOW = 1_800_000_000;

const aviora: Project = {
  id: 1,
  name: 'aviora',
  rootPath: '/home/dev/aviora',
  isGit: true,
  gitRoot: null,
  markers: ['node'],
  lastOpenedAt: NOW,
  createdAt: NOW,
  updatedAt: NOW,
};

function workspace(overrides: Partial<Workspace> = {}): Workspace {
  return {
    id: 1,
    projectId: 1,
    name: 'Web Development',
    description: null,
    applications: [],
    preferences: [],
    lastOpenedAt: null,
    createdAt: NOW,
    updatedAt: NOW,
    ...overrides,
  };
}

function snapshot({
  directoryExists = true,
  ports = [3000],
}: { directoryExists?: boolean; ports?: number[] } = {}): LiveSnapshot {
  return {
    projects: [
      {
        projectId: 1,
        directoryExists,
        git: {
          state: 'ready',
          head: { kind: 'branch', name: 'main' },
          clean: false,
          changed: 2,
          lastCommit: null,
          upstream: null,
        },
        layout: { kind: 'standalone' },
        error: null,
        observedAt: NOW,
      },
    ],
    services: {
      services: ports.map((port) => ({
        listener: { port, localAddress: '127.0.0.1', pid: 10 },
        process: {
          pid: 10,
          name: 'node',
          executable: null,
          parent: null,
          workingDirectory: '/home/dev/aviora',
        },
        attribution: { kind: 'project', projectId: 1, package: null },
      })),
      error: null,
      observedAt: NOW,
    },
  };
}

const installed: AppReport[] = [
  { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
  { kind: 'terminal', presence: { state: 'available', name: 'Ghostty' } },
  { kind: 'browser', presence: { state: 'available', name: 'Firefox' } },
];

const openable: AppReport[] = [
  { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
  { kind: 'terminal', presence: { state: 'available', name: 'Ghostty' } },
];

function backend({
  workspaces = [workspace()],
  canOpen = openable,
  live = snapshot(),
  onLaunch,
  chosen,
}: {
  workspaces?: Workspace[];
  canOpen?: AppReport[];
  live?: LiveSnapshot;
  onLaunch?: (kind: string) => { application: string | null } | Error;
  /** What a workspace's choice resolves to, when a test is about one. */
  chosen?: ChosenApp;
} = {}) {
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case 'projects_list':
        return Promise.resolve([aviora]);
      case 'live_refresh':
      case 'live_snapshot':
        return Promise.resolve(live);
      case 'workspaces_list':
        return Promise.resolve(workspaces);
      case 'workspaces_applications':
        return Promise.resolve(installed);
      case 'workspaces_openable':
        return Promise.resolve(canOpen);
      case 'workspaces_chosen':
        return Promise.resolve(chosen ?? { state: 'automatic', application: null });
      case 'workspaces_catalogue':
        return Promise.resolve({ kind: args?.['kind'], options: [], automatic: null });
      case 'workspaces_open':
        return Promise.resolve(
          workspaces.find((one) => one.id === args?.['workspaceId']) ?? workspaces[0],
        );
      case 'workspaces_launch': {
        const outcome = onLaunch?.(String(args?.['kind'])) ?? {
          application: 'Visual Studio Code',
        };
        return outcome instanceof Error
          ? Promise.reject({
              kind: 'unsupported',
              capability: 'launchApplication',
              reason: outcome.message,
            })
          : Promise.resolve(outcome);
      }
      default:
        return Promise.resolve(null);
    }
  });
}

/** Open the first workspace and land on its surface. */
async function openWorkspace(name = 'Web Development') {
  renderApp(<App surface="main" />);
  const list = await screen.findByRole('list', { name: /workspaces/i });
  await userEvent.click(within(list).getByRole('button', { name: new RegExp(name, 'i') }));
  return screen.findByRole('heading', { name });
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('open with', () => {
  it('offers each kind, naming what will actually open', async () => {
    backend();
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    expect(
      within(actions).getByRole('button', { name: /Visual Studio Code/ }),
    ).toBeInTheDocument();
    expect(within(actions).getByRole('button', { name: /Ghostty/ })).toBeInTheDocument();
  });

  it('asks for a kind, and sends nothing else', async () => {
    backend();
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Visual Studio Code/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_launch', {
        workspaceId: 1,
        kind: 'editor',
      }),
    );

    const sent = invoke.mock.calls
      .filter(([command]) => command === 'workspaces_launch')
      .map(([, args]) => Object.keys(args as object).sort());
    expect(sent).toEqual([['kind', 'workspaceId']]);
  });

  it('opens a terminal by its own kind', async () => {
    backend();
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Ghostty/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_launch', {
        workspaceId: 1,
        kind: 'terminal',
      }),
    );
  });

  it('confirms what was opened', async () => {
    backend({ onLaunch: () => ({ application: 'Ghostty' }) });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Ghostty/ }));

    expect(
      await within(actions.parentElement as HTMLElement).findByText(/opened ghostty/i),
    ).toBeInTheDocument();
  });
});

describe('an application that is not there', () => {
  it('says so instead of offering a button', async () => {
    backend({
      canOpen: [
        { kind: 'editor', presence: { state: 'notInstalled' } },
        { kind: 'terminal', presence: { state: 'available', name: 'Ghostty' } },
      ],
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    expect(within(actions).queryByRole('button', { name: /editor/i })).not.toBeInTheDocument();
    expect(within(actions).getByText(/editor/i)).toBeInTheDocument();
    expect(within(actions).getByRole('button', { name: /Ghostty/ })).toBeInTheDocument();
  });

  it('offers nothing at all when the machine has neither', async () => {
    backend({
      canOpen: [
        { kind: 'editor', presence: { state: 'notInstalled' } },
        { kind: 'terminal', presence: { state: 'notInstalled' } },
      ],
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    expect(within(actions).queryAllByRole('button')).toHaveLength(0);
  });
});

describe('a project whose folder is missing', () => {
  it('offers no way to open anything', async () => {
    backend({ live: snapshot({ directoryExists: false }) });
    await openWorkspace();

    expect(await screen.findByText(/folder is missing/i)).toBeInTheDocument();
    expect(screen.queryByRole('group', { name: /open with/i })).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /Visual Studio Code/ }),
    ).not.toBeInTheDocument();
  });

  it('keeps the workspace itself', async () => {
    backend({ live: snapshot({ directoryExists: false }) });
    await openWorkspace();

    expect(await screen.findByRole('heading', { name: 'Web Development' })).toBeInTheDocument();
  });
});

describe('a launch that fails', () => {
  it('shows the reason the backend gave', async () => {
    backend({
      onLaunch: () => new Error('Mira could not find an editor on this machine.'),
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Visual Studio Code/ }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/could not find an editor/i);
  });
});

describe('opening a service', () => {
  it('sends the port, never an address', async () => {
    backend();
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /services/i });
    await userEvent.click(within(services).getByRole('button', { name: /^open$/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('live_open_service', { port: 3000 }),
    );
  });

  it('has nothing to open when nothing is listening', async () => {
    backend({ live: snapshot({ ports: [] }) });
    await openWorkspace();

    await screen.findByRole('group', { name: /open with/i });
    expect(screen.queryByRole('list', { name: /services/i })).not.toBeInTheDocument();
  });
});

describe('more than one workspace', () => {
  const two = [workspace(), workspace({ id: 2, name: 'API Development' })];

  it('opens the workspace being shown, not the one opened first', async () => {
    // The bug this exists to prevent: an action that captured its workspace when
    // the surface first rendered would keep launching for the wrong one.
    backend({ workspaces: two });
    await openWorkspace('API Development');

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Visual Studio Code/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_launch', {
        workspaceId: 2,
        kind: 'editor',
      }),
    );
  });

  it('follows a switch between them', async () => {
    backend({ workspaces: two });
    await openWorkspace('Web Development');

    const list = screen.getByRole('list', { name: /workspaces/i });
    await userEvent.click(within(list).getByRole('button', { name: /API Development/i }));
    await screen.findByRole('heading', { name: 'API Development' });

    const actions = screen.getByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: /Ghostty/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_launch', {
        workspaceId: 2,
        kind: 'terminal',
      }),
    );
  });
});

// ── Opening what was chosen ──────────────────────────────────────────────────

describe('a workspace that chose its own application', () => {
  it('names the choice on the button rather than what Mira found first', async () => {
    // The lie this prevents: "Editor · Visual Studio Code" on a workspace that
    // chose Zed, where pressing it opens Zed.
    backend({
      workspaces: [workspace({ preferences: [{ kind: 'editor', application: 'zed' }] })],
      canOpen: [
        { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
      ],
      chosen: { state: 'ready', id: 'zed', name: 'Zed' },
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    expect(within(actions).getByRole('button', { name: 'Editor · Zed' })).toBeInTheDocument();
    expect(
      within(actions).queryByRole('button', { name: /Visual Studio Code/ }),
    ).not.toBeInTheDocument();
  });

  it('offers no button for a chosen application that is gone', async () => {
    // A button that could only fail, on a surface where the alternative would be
    // opening something else, is worse than a sentence.
    backend({
      workspaces: [workspace({ preferences: [{ kind: 'editor', application: 'zed' }] })],
      canOpen: [
        { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
      ],
      chosen: { state: 'missing', id: 'zed', name: 'Zed' },
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    expect(within(actions).getByText('Editor · Zed is not available here')).toBeInTheDocument();
    expect(within(actions).queryByRole('button')).not.toBeInTheDocument();
  });

  it('still sends only a workspace and a kind when it opens', async () => {
    backend({
      workspaces: [workspace({ preferences: [{ kind: 'editor', application: 'zed' }] })],
      canOpen: [
        { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
      ],
      chosen: { state: 'ready', id: 'zed', name: 'Zed' },
    });
    await openWorkspace();

    const actions = await screen.findByRole('group', { name: /open with/i });
    await userEvent.click(within(actions).getByRole('button', { name: 'Editor · Zed' }));

    await waitFor(() => {
      const asked = invoke.mock.calls.find(([command]) => command === 'workspaces_launch');
      expect(asked?.[1]).toEqual({ workspaceId: 1, kind: 'editor' });
    });
  });
});
