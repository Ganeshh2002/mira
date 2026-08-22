import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppReport } from '../bindings/AppReport';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { ServiceOffer } from '../bindings/ServiceOffer';
import type { Workspace } from '../bindings/Workspace';
import type { WorkspaceService } from '../bindings/WorkspaceService';
import { App } from '../App';
import { renderApp } from '../test/render';

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

const live: LiveSnapshot = {
  projects: [
    {
      projectId: 1,
      directoryExists: true,
      git: {
        state: 'ready',
        head: { kind: 'branch', name: 'main' },
        clean: false,
        changed: 2,
        lastCommit: null,
        upstream: null,
      },
      layout: {
        kind: 'monorepoRoot',
        tools: ['pnpmWorkspaces'],
        packages: [
          { name: '@aviora/web', path: 'apps/web' },
          { name: '@aviora/api', path: 'apps/api' },
        ],
      },
      error: null,
      observedAt: NOW,
    },
  ],
  services: {
    services: [
      {
        listener: { port: 3000, localAddress: '127.0.0.1', pid: 10 },
        process: {
          pid: 10,
          name: 'node',
          executable: null,
          parent: null,
          workingDirectory: '/home/dev/aviora/apps/web',
        },
        attribution: {
          kind: 'project',
          projectId: 1,
          package: { name: '@aviora/web', path: 'apps/web' },
        },
      },
    ],
    error: null,
    observedAt: NOW,
  },
};

const everythingInstalled: AppReport[] = [
  { kind: 'editor', presence: { state: 'available', name: 'Visual Studio Code' } },
  { kind: 'terminal', presence: { state: 'available', name: 'Ghostty' } },
  { kind: 'browser', presence: { state: 'available', name: 'Firefox' } },
];

/** Serve projects, live state, workspaces, and application availability. */
/** A watched service that is up, as the backend would resolve it. */
function running(port: number, workspaceId: number): WorkspaceService {
  return {
    watched: { id: port, workspaceId, port, addedAt: 1_800_000_000 },
    state: { kind: 'running', address: '127.0.0.1', process: 'node', pid: 18234 },
  };
}

function backend({
  workspaces = [] as Workspace[],
  apps = everythingInstalled,
  projects = [aviora],
  onCreate,
  watching,
  offers = [
    { at: 0, port: 3000, address: '127.0.0.1', process: 'node', watched: false },
    { at: 1, port: 5173, address: '127.0.0.1', process: 'vite', watched: false },
  ],
}: {
  workspaces?: Workspace[];
  apps?: AppReport[];
  projects?: Project[];
  onCreate?: (name: string) => Workspace | Error;
  watching?: Map<number, WorkspaceService[]>;
  offers?: ServiceOffer[];
} = {}) {
  let listed = workspaces;
  // What each workspace watches, keyed by workspace id. Stateful, because
  // adding and removing a service is the thing under test and a stub that
  // always returned the same list could not show either working.
  const watched = new Map<number, WorkspaceService[]>(watching ? [...watching.entries()] : []);

  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    const forWorkspace = () => watched.get(Number(args?.['workspaceId'])) ?? [];

    switch (command) {
      case 'projects_list':
        return Promise.resolve(projects);
      case 'live_refresh':
      case 'live_snapshot':
        return Promise.resolve(live);
      case 'workspaces_list':
        return Promise.resolve(listed);
      case 'workspaces_services':
        return Promise.resolve(forWorkspace());
      case 'workspaces_service_offers':
        return Promise.resolve(offers);
      case 'workspaces_watch_service': {
        const offer = offers[Number(args?.['at'])];
        if (!offer) return Promise.reject({ kind: 'notFound', what: 'That service' });
        const id = Number(args?.['workspaceId']);
        const next = [...(watched.get(id) ?? []), running(offer.port, id)];
        watched.set(id, next);
        return Promise.resolve(next);
      }
      case 'workspaces_forget_service': {
        const id = Number(args?.['workspaceId']);
        const next = (watched.get(id) ?? []).filter(
          (service) => service.watched.id !== Number(args?.['serviceId']),
        );
        watched.set(id, next);
        return Promise.resolve(next);
      }
      case 'workspaces_applications':
        return Promise.resolve(apps);
      case 'workspaces_openable':
        return Promise.resolve(apps.filter((report) => report.kind !== 'browser'));
      case 'workspaces_create': {
        const made =
          onCreate?.(String(args?.['name'])) ??
          workspace({ id: 9, name: String(args?.['name']) });
        if (made instanceof Error)
          return Promise.reject({ kind: 'invalid', field: 'name', detail: made.message });
        listed = [...listed, made];
        return Promise.resolve(made);
      }
      case 'workspaces_open':
        return Promise.resolve(listed.find((w) => w.id === args?.['workspaceId']) ?? listed[0]);
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('a project with no workspaces', () => {
  it('says so and offers the one action', async () => {
    backend();
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/no workspaces yet/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /new workspace/i })).toBeInTheDocument();
  });

  it('still shows the project itself', async () => {
    backend();
    renderApp(<App surface="main" />);

    expect(await screen.findByRole('heading', { name: 'aviora' })).toBeInTheDocument();
    expect(screen.getByText('main')).toBeInTheDocument();
  });
});

describe('creating a workspace', () => {
  it('needs only a name', async () => {
    backend();
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /new workspace/i }));
    await userEvent.type(screen.getByRole('textbox', { name: /name/i }), 'Web Development');
    await userEvent.click(screen.getByRole('button', { name: /^create$/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_create', {
        projectId: 1,
        name: 'Web Development',
        description: null,
      }),
    );
  });

  it('shows the new workspace without a reload', async () => {
    backend();
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /new workspace/i }));
    await userEvent.type(screen.getByRole('textbox', { name: /name/i }), 'Backend');
    await userEvent.click(screen.getByRole('button', { name: /^create$/i }));

    const list = await screen.findByRole('list', { name: /workspaces/i });
    expect(within(list).getByText('Backend')).toBeInTheDocument();
  });

  it('explains a name the project already uses', async () => {
    backend({
      workspaces: [workspace()],
      onCreate: () =>
        new Error('This project already has a workspace called "Web Development".'),
    });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /new workspace/i }));
    await userEvent.type(screen.getByRole('textbox', { name: /name/i }), 'Web Development');
    await userEvent.click(screen.getByRole('button', { name: /^create$/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/already has a workspace/i);
  });

  it('will not submit an empty name', async () => {
    backend();
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /new workspace/i }));
    await userEvent.click(screen.getByRole('button', { name: /^create$/i }));

    expect(invoke).not.toHaveBeenCalledWith('workspaces_create', expect.anything());
  });
});

describe('workspace navigation', () => {
  const three = [
    workspace({ id: 1, name: 'Web Development' }),
    workspace({ id: 2, name: 'Mobile Development' }),
    workspace({ id: 3, name: 'API Development' }),
  ];

  it('lists every workspace of the project', async () => {
    backend({ workspaces: three });
    renderApp(<App surface="main" />);

    const list = await screen.findByRole('list', { name: /workspaces/i });
    expect(within(list).getAllByRole('listitem')).toHaveLength(3);
  });

  it('opens the one that was chosen', async () => {
    backend({ workspaces: three });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Mobile Development/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('workspaces_open', { workspaceId: 2 }),
    );
  });

  it('shows the chosen workspace, and only it', async () => {
    backend({ workspaces: three });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /API Development/ }));

    expect(await screen.findByRole('heading', { name: /API Development/ })).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: /Mobile Development/ }),
    ).not.toBeInTheDocument();
  });

  it('reads the same project context whichever workspace is open', async () => {
    // Slice brief §12: workspaces share the project's observations rather than
    // each starting their own. Switching must not cost another observation.
    backend({ workspaces: three });
    renderApp(<App surface="main" />);

    await screen.findByRole('list', { name: /workspaces/i });
    const before = invoke.mock.calls.filter(([c]) => c === 'live_refresh').length;

    await userEvent.click(screen.getByRole('button', { name: /Mobile Development/ }));
    await screen.findByRole('heading', { name: /Mobile Development/ });

    const after = invoke.mock.calls.filter(([c]) => c === 'live_refresh').length;
    expect(after).toBe(before);
  });
});

describe('the workspace surface', () => {
  const web = workspace({
    id: 1,
    name: 'Web Development',
    applications: ['editor', 'browser'],
  });

  it('shows the project it belongs to', async () => {
    backend({ workspaces: [web] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    expect(await screen.findByText('/home/dev/aviora')).toBeInTheDocument();
  });

  it('shows the packages the repository declares', async () => {
    backend({ workspaces: [web] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    const packages = await screen.findByRole('list', { name: /packages/i });
    expect(within(packages).getByText('apps/web')).toBeInTheDocument();
    expect(within(packages).getByText('apps/api')).toBeInTheDocument();
  });

  it('shows the project Git state', async () => {
    backend({ workspaces: [web] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    expect(await screen.findByText('main')).toBeInTheDocument();
    expect(screen.getByText(/2 changed/i)).toBeInTheDocument();
  });

  it("shows the services this workspace watches, and not the project's others", async () => {
    // Slice 4c is the narrowing ADR-0012 said was missing. The project is
    // serving :3000 and :5173; this workspace said one of them is the work, and
    // the surface shows one row (ADR-0020).
    backend({ workspaces: [web], watching: new Map([[web.id, [running(3000, web.id)]]]) });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    const services = await screen.findByRole('list', { name: /watched services/i });
    expect(within(services).getByText(':3000')).toBeInTheDocument();
    expect(within(services).queryByText(':5173')).not.toBeInTheDocument();
  });

  it('says a workspace watches nothing rather than showing an empty box', async () => {
    backend({ workspaces: [web] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    expect(await screen.findByText(/not watching any services yet/i)).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /watched services/i })).not.toBeInTheDocument();
  });
});

describe('application context', () => {
  it('names the application it found for each kind', async () => {
    backend({ workspaces: [workspace({ applications: ['editor', 'terminal'] })] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    const context = await screen.findByRole('list', { name: /context/i });
    expect(within(context).getByText('Visual Studio Code')).toBeInTheDocument();
    expect(within(context).getByText('Ghostty')).toBeInTheDocument();
  });

  it('says an application is not installed rather than hiding the association', async () => {
    backend({
      workspaces: [workspace({ applications: ['editor'] })],
      apps: [
        { kind: 'editor', presence: { state: 'notInstalled' } },
        { kind: 'terminal', presence: { state: 'available', name: 'Ghostty' } },
        { kind: 'browser', presence: { state: 'available', name: 'Firefox' } },
      ],
    });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));

    const context = await screen.findByRole('list', { name: /context/i });
    // The row's own label, not the chooser's — the chooser beside it is named
    // "Editor application" and would match a looser pattern.
    expect(within(context).getByText('Editor')).toBeInTheDocument();
    expect(within(context).getByText(/not installed/i)).toBeInTheDocument();
  });

  it('keeps what was already associated when another kind is added', async () => {
    // The bug this test exists for: the detail view held the workspace it was
    // handed at selection time. After a toggle the stored value changed and the
    // held copy did not, so the *next* toggle computed its new list from a stale
    // one and silently dropped what the first had added.
    let stored: Workspace = workspace({ applications: [] });
    invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'projects_list') return Promise.resolve([aviora]);
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(live);
      if (command === 'workspaces_list') return Promise.resolve([stored]);
      if (command === 'workspaces_applications') return Promise.resolve(everythingInstalled);
      if (command === 'workspaces_openable') return Promise.resolve([]);
      if (command === 'workspaces_set_applications') {
        stored = { ...stored, applications: args?.['kinds'] as Workspace['applications'] };
        return Promise.resolve(stored);
      }
      if (
        command === 'workspaces_services' ||
        command === 'workspaces_service_offers' ||
        command === 'workspaces_actions' ||
        command === 'workspaces_action_catalogue'
      )
        return Promise.resolve([]);
      return Promise.resolve(stored);
    });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));
    const context = await screen.findByRole('list', { name: /context/i });
    const boxes = within(context).getAllByRole('checkbox');

    await userEvent.click(boxes[2] as HTMLElement); // browser
    await waitFor(() => expect(stored.applications).toEqual(['browser']));

    await userEvent.click(boxes[0] as HTMLElement); // editor

    await waitFor(() =>
      expect(stored.applications).toEqual(expect.arrayContaining(['browser', 'editor'])),
    );
  });

  it('offers no way to restore or start anything', async () => {
    backend({ workspaces: [workspace({ applications: ['editor'] })] });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Web Development/ }));
    await screen.findByRole('list', { name: /context/i });

    // Opening an editor is a real action now, and it is the *only* kind of
    // starting there is. Nothing restores a workspace, nothing starts a
    // development server, and there is no disabled button hinting that it
    // nearly does (`information-architecture.md` §5).
    for (const absent of [/launch/i, /start editor/i, /restore/i]) {
      expect(screen.queryByRole('button', { name: absent })).not.toBeInTheDocument();
    }
  });
});

describe('a project whose folder is gone', () => {
  it('keeps its workspaces and shows the missing folder', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'projects_list') return Promise.resolve([aviora]);
      if (command === 'workspaces_list') return Promise.resolve([workspace()]);
      if (command === 'workspaces_applications') return Promise.resolve(everythingInstalled);
      if (command === 'workspaces_openable') return Promise.resolve([]);
      if (command === 'live_refresh' || command === 'live_snapshot') {
        return Promise.resolve({
          projects: [
            {
              projectId: 1,
              directoryExists: false,
              git: null,
              layout: null,
              error: null,
              observedAt: NOW,
            },
          ],
          services: { services: [], error: null, observedAt: NOW },
        });
      }
      return Promise.resolve(null);
    });
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/folder is missing/i)).toBeInTheDocument();
    // FR-1.5 and slice brief §13: the workspaces are stated, the folder is
    // observed. A missing folder never silently deletes anything.
    const list = await screen.findByRole('list', { name: /workspaces/i });
    expect(within(list).getByText('Web Development')).toBeInTheDocument();
  });
});
