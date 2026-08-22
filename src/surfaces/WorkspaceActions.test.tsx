import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppReport } from '../bindings/AppReport';
import type { ChosenApp } from '../bindings/ChosenApp';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { ActionOffer } from '../bindings/ActionOffer';
import type { ActionState } from '../bindings/ActionState';
import type { ServiceOffer } from '../bindings/ServiceOffer';
import type { ServiceState } from '../bindings/ServiceState';
import type { Workspace } from '../bindings/Workspace';
import type { WorkspaceAction } from '../bindings/WorkspaceAction';
import type { WorkspaceService } from '../bindings/WorkspaceService';
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
          cpuShare: null,
          memoryBytes: null,
          uptimeSeconds: null,
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

/** One service a workspace watches, in whatever state the test needs. */
function watched(
  port: number,
  state: ServiceState = {
    kind: 'running',
    address: '127.0.0.1',
    process: 'node',
    pid: 18234,
    cpuShare: 2.4,
    memoryBytes: 188_743_680,
    uptimeSeconds: 3_600,
  },
  workspaceId = 1,
): WorkspaceService {
  return {
    watched: { id: port, workspaceId, port, addedAt: 1_800_000_000 },
    state,
  };
}

/** The catalogue, as the backend would describe it. */
const CATALOGUE: ActionOffer[] = [
  {
    id: 'open-editor',
    label: 'Open in the editor',
    describes: "Opens this project's folder in the editor this workspace uses.",
    icon: 'editor',
    chosen: false,
  },
  {
    id: 'open-service',
    label: 'Open the running service',
    describes: "Opens this workspace's running service in the browser, when exactly one is up.",
    icon: 'service',
    chosen: false,
  },
  {
    id: 'refresh',
    label: 'Read everything again',
    describes: "Reads this project's Git state and the machine's ports now.",
    icon: 'refresh',
    chosen: false,
  },
];

/** One of a workspace's actions, in whatever state the test needs. */
function action(
  id: string,
  state: ActionState = { kind: 'ready', detail: null },
): WorkspaceAction {
  const row = CATALOGUE.find((offer) => offer.id === id);
  return {
    id,
    label: row?.label ?? '',
    describes: row?.describes ?? '',
    icon: row?.icon ?? '',
    effect: row ? { does: 'observe' } : null,
    state,
  };
}

function backend({
  workspaces = [workspace()],
  canOpen = openable,
  live = snapshot(),
  onLaunch,
  chosen,
  watching = new Map<number, WorkspaceService[]>(),
  doing = new Map<number, WorkspaceAction[]>(),
  offers = [
    { at: 0, port: 3000, address: '127.0.0.1', process: 'node', watched: false },
    { at: 1, port: 5173, address: '127.0.0.1', process: 'vite', watched: false },
  ] as ServiceOffer[],
}: {
  workspaces?: Workspace[];
  canOpen?: AppReport[];
  live?: LiveSnapshot;
  onLaunch?: (kind: string) => { application: string | null } | Error;
  /** What a workspace's choice resolves to, when a test is about one. */
  chosen?: ChosenApp;
  watching?: Map<number, WorkspaceService[]>;
  offers?: ServiceOffer[];
  doing?: Map<number, WorkspaceAction[]>;
} = {}) {
  const lists = new Map(watching);
  const acts = new Map(doing);
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
      case 'workspaces_services':
        return Promise.resolve(lists.get(Number(args?.['workspaceId'])) ?? []);
      case 'workspaces_service_offers':
        return Promise.resolve(offers);
      case 'workspaces_watch_service': {
        const offer = offers[Number(args?.['at'])];
        if (!offer) return Promise.reject({ kind: 'notFound', what: 'That service' });
        const id = Number(args?.['workspaceId']);
        const next = [...(lists.get(id) ?? []), watched(offer.port, undefined, id)];
        lists.set(id, next);
        return Promise.resolve(next);
      }
      case 'workspaces_forget_service': {
        const id = Number(args?.['workspaceId']);
        const next = (lists.get(id) ?? []).filter(
          (service) => service.watched.id !== Number(args?.['serviceId']),
        );
        lists.set(id, next);
        return Promise.resolve(next);
      }
      case 'workspaces_open_service':
        return Promise.resolve({ application: 'Firefox' });
      case 'workspaces_actions':
        return Promise.resolve(acts.get(Number(args?.['workspaceId'])) ?? []);
      case 'workspaces_action_catalogue': {
        const has = acts.get(Number(args?.['workspaceId'])) ?? [];
        return Promise.resolve(
          CATALOGUE.map((offer) => ({
            ...offer,
            chosen: has.some((one) => one.id === offer.id),
          })),
        );
      }
      case 'workspaces_set_action': {
        const id = Number(args?.['workspaceId']);
        const named = String(args?.['action']);
        const has = acts.get(id) ?? [];
        acts.set(
          id,
          args?.['wanted'] ? [...has, action(named)] : has.filter((one) => one.id !== named),
        );
        return Promise.resolve(acts.get(id));
      }
      case 'workspaces_perform_action':
        return Promise.resolve({
          id: String(args?.['action']),
          happened: 'Opened in Visual Studio Code.',
        });
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

describe('the services a workspace watches', () => {
  it('shows only what this workspace chose, not everything the project runs', async () => {
    // The narrowing. The project is serving :3000 and :5173; this workspace said
    // one of them is the work (ADR-0020).
    backend({ watching: new Map([[1, [watched(3000)]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText(':3000')).toBeInTheDocument();
    expect(within(services).queryByText(':5173')).not.toBeInTheDocument();
  });

  it('opens a service by the id Mira issued, never by a port or an address', async () => {
    backend({ watching: new Map([[1, [watched(3000)]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    await userEvent.click(within(services).getByRole('button', { name: /^open$/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_open_service', {
        workspaceId: 1,
        serviceId: 3000,
      }),
    );

    const sent = invoke.mock.calls.find(([name]) => name === 'workspaces_open_service')?.[1];
    expect(JSON.stringify(sent)).not.toMatch(/http|localhost|127\.0\.0\.1/);
  });

  it('adds a service by its position in the list Mira offered', async () => {
    // No port on the wire. The menu shows what Mira found and sends back where
    // it sat in that list.
    backend();
    await openWorkspace();

    await userEvent.click(await screen.findByRole('button', { name: /add a service/i }));
    await userEvent.click(await screen.findByRole('menuitem', { name: /5173/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_watch_service', {
        workspaceId: 1,
        at: 1,
      }),
    );

    const sent = invoke.mock.calls.find(([name]) => name === 'workspaces_watch_service')?.[1];
    expect(sent).toEqual({ workspaceId: 1, at: 1 });
  });

  it('removes a service by the id Mira issued', async () => {
    backend({ watching: new Map([[1, [watched(3000)]]]) });
    await openWorkspace();

    await userEvent.click(
      await screen.findByRole('button', { name: /stop watching port 3000/i }),
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_forget_service', {
        workspaceId: 1,
        serviceId: 3000,
      }),
    );
  });

  it('says a workspace watches nothing rather than showing an empty list', async () => {
    backend();
    await openWorkspace();

    expect(await screen.findByText(/not watching any services yet/i)).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /watched services/i })).not.toBeInTheDocument();
  });

  it('offers nothing when Mira has not seen the project serving anything', async () => {
    backend({ offers: [] });
    await openWorkspace();

    await userEvent.click(await screen.findByRole('button', { name: /add a service/i }));
    expect(
      await screen.findByText(/has not seen this project serving anything/i),
    ).toBeInTheDocument();
  });

  it('does not offer a service this workspace already watches', async () => {
    backend({
      watching: new Map([[1, [watched(3000)]]]),
      offers: [
        { at: 0, port: 3000, address: '127.0.0.1', process: 'node', watched: true },
        { at: 1, port: 5173, address: '127.0.0.1', process: 'vite', watched: false },
      ],
    });
    await openWorkspace();

    await userEvent.click(await screen.findByRole('button', { name: /add a service/i }));
    expect(await screen.findByRole('menuitem', { name: /5173/ })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: /3000/ })).not.toBeInTheDocument();
  });
});

describe('two workspaces on one project', () => {
  const two = [workspace(), workspace({ id: 2, name: 'API Development' })];

  it("each shows its own services and neither shows the other's", async () => {
    // Same project, same observations, two different answers to "which of these
    // matter". The whole point of the slice, from the outside.
    backend({
      workspaces: two,
      watching: new Map([
        [1, [watched(3000, undefined, 1)]],
        [2, [watched(5173, undefined, 2)]],
      ]),
    });
    await openWorkspace('API Development');

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText(':5173')).toBeInTheDocument();
    expect(within(services).queryByText(':3000')).not.toBeInTheDocument();
  });

  it('asks about the workspace being shown, not the one opened first', async () => {
    backend({ workspaces: two, watching: new Map([[2, [watched(5173, undefined, 2)]]]) });
    await openWorkspace('API Development');

    await screen.findByRole('list', { name: /watched services/i });
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_services', { workspaceId: 2 }),
    );
  });

  it('removes from the workspace being shown', async () => {
    backend({ workspaces: two, watching: new Map([[2, [watched(5173, undefined, 2)]]]) });
    await openWorkspace('API Development');

    await userEvent.click(
      await screen.findByRole('button', { name: /stop watching port 5173/i }),
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_forget_service', {
        workspaceId: 2,
        serviceId: 5173,
      }),
    );
  });
});

describe('what a watched service says about itself', () => {
  it('a running service is named as running, with what is behind it', async () => {
    backend({ watching: new Map([[1, [watched(3000)]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText('Running')).toBeInTheDocument();
    expect(within(services).getByText(/node/)).toBeInTheDocument();
    expect(within(services).getByText(/listening on 127\.0\.0\.1/i)).toBeInTheDocument();
  });

  it('an expected service that is down reads differently from one that is up', async () => {
    // The requirement, as a test: the two states must be distinguishable, and
    // the down one must not offer an Open button that would fail.
    backend({ watching: new Map([[1, [watched(3000, { kind: 'notRunning' })]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText('Not running')).toBeInTheDocument();
    expect(within(services).queryByRole('button', { name: /^open$/i })).not.toBeInTheDocument();
    expect(within(services).getByText(':3000')).toBeInTheDocument();
  });

  it('never offers to start or stop the process behind a service', async () => {
    backend({ watching: new Map([[1, [watched(3000)]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    for (const destructive of [/kill/i, /terminate/i, /restart/i, /^start$/i, /^stop$/i]) {
      expect(
        within(services).queryByRole('button', { name: destructive }),
      ).not.toBeInTheDocument();
    }
  });

  it('a port something else took is never shown as running', async () => {
    // The substitution rule. Reporting a stranger's process as your dev server
    // would invite somebody to open it.
    backend({
      watching: new Map([[1, [watched(3000, { kind: 'taken', process: 'postgres' })]]]),
    });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText('Port taken')).toBeInTheDocument();
    expect(within(services).getByText(/postgres/)).toBeInTheDocument();
    expect(within(services).queryByText('Running')).not.toBeInTheDocument();
    expect(within(services).queryByRole('button', { name: /^open$/i })).not.toBeInTheDocument();
  });

  it('says Mira has not looked rather than saying nothing is running', async () => {
    backend({ watching: new Map([[1, [watched(3000, { kind: 'neverObserved' })]]]) });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText('Never observed')).toBeInTheDocument();
    expect(within(services).queryByText('Not running')).not.toBeInTheDocument();
  });

  it('says the reading failed, and what the platform said', async () => {
    backend({
      watching: new Map([
        [1, [watched(3000, { kind: 'unreadable', reason: 'Refused by the platform.' })]],
      ]),
    });
    await openWorkspace();

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText('Cannot tell')).toBeInTheDocument();
    expect(within(services).getByText(/refused by the platform/i)).toBeInTheDocument();
    expect(within(services).queryByText('Not running')).not.toBeInTheDocument();
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

describe('the actions a workspace has', () => {
  it('says a workspace has none rather than showing an empty list', async () => {
    backend();
    await openWorkspace();

    expect(await screen.findByText(/no actions yet/i)).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /workspace actions/i })).not.toBeInTheDocument();
  });

  it('adds an action by the identity Mira offered, never by text', async () => {
    backend();
    await openWorkspace();

    await userEvent.click(await screen.findByRole('button', { name: /add an action/i }));
    await userEvent.click(await screen.findByRole('menuitem', { name: /open in the editor/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_set_action', {
        workspaceId: 1,
        action: 'open-editor',
        wanted: true,
      }),
    );

    // Nothing runnable crossed the boundary: the payload is an identity.
    const sent = invoke.mock.calls.find(([name]) => name === 'workspaces_set_action')?.[1];
    expect(sent).toEqual({ workspaceId: 1, action: 'open-editor', wanted: true });
    expect(JSON.stringify(sent)).not.toMatch(/npm|cargo|sh|--|\//);
  });

  it('performs an action by its identity and says what happened', async () => {
    backend({ doing: new Map([[1, [action('open-editor')]]]) });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    await userEvent.click(within(list).getByRole('button', { name: 'Open in the editor' }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_perform_action', {
        workspaceId: 1,
        action: 'open-editor',
      }),
    );
    expect(await screen.findByText(/opened in visual studio code/i)).toBeInTheDocument();
  });

  it('removes an action by its identity', async () => {
    backend({ doing: new Map([[1, [action('refresh')]]]) });
    await openWorkspace();

    await userEvent.click(
      await screen.findByRole('button', { name: /remove read everything again/i }),
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_set_action', {
        workspaceId: 1,
        action: 'refresh',
        wanted: false,
      }),
    );
  });

  it('shows each action with a sentence saying what it will do', async () => {
    // The safety story, made visible. A row nobody can read is a row somebody
    // presses without knowing what happens.
    backend({ doing: new Map([[1, [action('open-editor')]]]) });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    expect(
      within(list).getByText(/opens this project's folder in the editor/i),
    ).toBeInTheDocument();
  });

  it('does not offer an action this workspace already has', async () => {
    backend({ doing: new Map([[1, [action('refresh')]]]) });
    await openWorkspace();

    await userEvent.click(await screen.findByRole('button', { name: /add an action/i }));
    expect(
      await screen.findByRole('menuitem', { name: /open in the editor/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: /read everything again/i }),
    ).not.toBeInTheDocument();
  });

  it('offers no way to run, stop or restart anything', async () => {
    backend({ doing: new Map([[1, [action('open-editor'), action('refresh')]]]) });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    for (const destructive of [/kill/i, /terminate/i, /^stop$/i, /^restart$/i, /^run$/i]) {
      expect(within(list).queryByRole('button', { name: destructive })).not.toBeInTheDocument();
    }
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});

describe('an action that cannot be done', () => {
  it('is a sentence rather than a disabled button', async () => {
    backend({
      doing: new Map([
        [
          1,
          [
            action('open-editor', {
              kind: 'unavailable',
              reason: 'There is no editor on this machine that Mira can open a folder in.',
            }),
          ],
        ],
      ]),
    });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    expect(within(list).getByText(/no editor on this machine/i)).toBeInTheDocument();
    expect(
      within(list).queryByRole('button', { name: 'Open in the editor' }),
    ).not.toBeInTheDocument();
  });

  it('is refused rather than guessed at when it would be ambiguous', async () => {
    // Two of this workspace's services running, and no way to know which one
    // "open the running service" meant. Mira says so instead of choosing.
    backend({
      doing: new Map([
        [
          1,
          [
            action('open-service', {
              kind: 'unavailable',
              reason:
                "2 of this workspace's services are running, so this action cannot say which one you mean. Open the one you want from the Services list.",
            }),
          ],
        ],
      ]),
    });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    expect(within(list).getByText(/cannot say which one you mean/i)).toBeInTheDocument();
    expect(
      within(list).queryByRole('button', { name: 'Open the running service' }),
    ).not.toBeInTheDocument();
  });

  it('names an identity Mira no longer has, and never a different action', async () => {
    backend({
      doing: new Map([
        [
          1,
          [
            {
              id: 'an-action-mira-removed',
              label: '',
              describes: '',
              icon: '',
              effect: null,
              state: { kind: 'unknown' },
            },
          ],
        ],
      ]),
    });
    await openWorkspace();

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    expect(within(list).getByText('an-action-mira-removed')).toBeInTheDocument();
    expect(within(list).getByText(/no action by that name/i)).toBeInTheDocument();
    // It offers removal, not performance — and it did not become another action.
    expect(
      within(list).getByRole('button', { name: /remove an-action-mira-removed/i }),
    ).toBeInTheDocument();
    expect(within(list).queryByText(/open in the editor/i)).not.toBeInTheDocument();
  });
});

describe('two workspaces and their actions', () => {
  const two = [workspace(), workspace({ id: 2, name: 'API Development' })];

  it("each shows its own and neither shows the other's", async () => {
    backend({
      workspaces: two,
      doing: new Map([
        [1, [action('open-editor')]],
        [2, [action('refresh')]],
      ]),
    });
    await openWorkspace('API Development');

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    expect(within(list).getByText('Read everything again')).toBeInTheDocument();
    expect(within(list).queryByText('Open in the editor')).not.toBeInTheDocument();
  });

  it('performs against the workspace being shown', async () => {
    backend({ workspaces: two, doing: new Map([[2, [action('refresh')]]]) });
    await openWorkspace('API Development');

    const list = await screen.findByRole('list', { name: /workspace actions/i });
    await userEvent.click(within(list).getByRole('button', { name: 'Read everything again' }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_perform_action', {
        workspaceId: 2,
        action: 'refresh',
      }),
    );
  });
});
