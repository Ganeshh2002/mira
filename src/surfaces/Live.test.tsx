import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { ProjectObservation } from '../bindings/ProjectObservation';
import type { Service } from '../bindings/Service';
import { App } from '../App';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const NOW = 1_800_000_000;

function project(overrides: Partial<Project> = {}): Project {
  return {
    id: 1,
    name: 'aviora',
    rootPath: '/home/dev/aviora',
    isGit: true,
    gitRoot: null,
    markers: ['node'],
    lastOpenedAt: NOW,
    createdAt: NOW,
    updatedAt: NOW,
    ...overrides,
  };
}

function observation(overrides: Partial<ProjectObservation> = {}): ProjectObservation {
  return {
    projectId: 1,
    directoryExists: true,
    git: {
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: null,
      upstream: null,
    },
    layout: { kind: 'standalone' },
    error: null,
    observedAt: NOW,
    ...overrides,
  };
}

function service(overrides: Partial<Service> = {}): Service {
  return {
    listener: { port: 3000, localAddress: '127.0.0.1', pid: 18234 },
    process: {
      pid: 18234,
      name: 'node',
      executable: '/usr/local/bin/node',
      parent: 900,
      workingDirectory: '/home/dev/aviora',
    },
    attribution: { kind: 'project', projectId: 1, package: null },
    ...overrides,
  };
}

function snapshot(overrides: Partial<LiveSnapshot> = {}): LiveSnapshot {
  return {
    projects: [observation()],
    services: { services: [service()], error: null, observedAt: NOW },
    ...overrides,
  };
}

/** Serve a project list and a live snapshot; remember the event handler. */
function backend(projects: Project[], live: LiveSnapshot) {
  invoke.mockImplementation((command: string) => {
    if (command === 'workspaces_list') return Promise.resolve([]);
    if (command === 'workspaces_applications') return Promise.resolve([]);
    if (command === 'projects_list') return Promise.resolve(projects);
    if (command === 'live_snapshot' || command === 'live_refresh') return Promise.resolve(live);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
  vi.setSystemTime(new Date(NOW * 1000));
});

describe('live Git state', () => {
  it('shows what the observers last saw, without asking for it again', async () => {
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    expect(await screen.findByText('main')).toBeInTheDocument();
    expect(screen.getByText(/clean/i)).toBeInTheDocument();
    // Slice 1 read Git inside the detail view. It is observed now, so there is
    // no per-project Git command left to call.
    const called = invoke.mock.calls.map((call) => String(call[0]));
    expect(called).not.toContain('projects_context');
  });

  it('updates when the backend says something moved', async () => {
    let current = snapshot();
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([project()]);
      return Promise.resolve(current);
    });
    renderApp(<App surface="main" />);

    await screen.findByText(/clean/i);

    current = snapshot({
      projects: [
        observation({
          git: {
            state: 'ready',
            head: { kind: 'branch', name: 'main' },
            clean: false,
            changed: 2,
            lastCommit: null,
            upstream: null,
          },
        }),
      ],
    });

    // The backend pushes a change notification carrying no payload; the
    // interface asks for what it needs (architecture.md §5).
    const handler = listen.mock.calls.find(([name]) => name === 'mira://live')?.[1] as
      (() => void) | undefined;
    expect(handler).toBeDefined();
    handler?.();

    expect(await screen.findByText(/2 changed/i)).toBeInTheDocument();
  });

  it('stops listening when the view goes away', async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);
    backend([project()], snapshot());
    const { unmount } = renderApp(<App surface="main" />);

    await screen.findByText('main');
    unmount();

    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });
});

describe('the dirty indicator', () => {
  it('marks each project in the list by what was observed', async () => {
    backend(
      [project({ id: 1, name: 'aviora' }), project({ id: 2, name: 'mobile', rootPath: '/b' })],
      snapshot({
        projects: [observation({ projectId: 1, git: dirty(3) }), observation({ projectId: 2 })],
      }),
    );
    renderApp(<App surface="main" />);

    const list = await screen.findByRole('list', { name: /projects/i });
    const rows = within(list).getAllByRole('listitem');

    expect(within(rows[0] as HTMLElement).getByRole('img')).toHaveAccessibleName(/changed/i);
    expect(within(rows[1] as HTMLElement).getByRole('img')).toHaveAccessibleName(/clean/i);
  });

  it('says nothing about a project it has not observed yet', async () => {
    backend([project()], snapshot({ projects: [] }));
    renderApp(<App surface="main" />);

    const list = await screen.findByRole('list', { name: /projects/i });
    const mark = within(list).getByRole('img');

    expect(mark).toHaveAccessibleName(/not.*observed|unknown/i);
  });
});

function dirty(changed: number): ProjectObservation['git'] {
  return {
    state: 'ready',
    head: { kind: 'branch', name: 'main' },
    clean: false,
    changed,
    lastCommit: null,
    upstream: null,
  };
}

describe('services', () => {
  it('lists what is listening, with its port and process', async () => {
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    const services = await screen.findByRole('list', { name: /services/i });

    expect(within(services).getByText(':3000')).toBeInTheDocument();
    expect(within(services).getByText(/node/)).toBeInTheDocument();
    expect(within(services).getByText(/18234/)).toBeInTheDocument();
  });

  it('groups a monorepo service under the package it runs from', async () => {
    backend(
      [project()],
      snapshot({
        services: {
          services: [
            service({
              listener: { port: 3000, localAddress: '127.0.0.1', pid: 1 },
              attribution: {
                kind: 'project',
                projectId: 1,
                package: { name: '@aviora/web', path: 'apps/web' },
              },
            }),
            service({
              listener: { port: 3001, localAddress: '127.0.0.1', pid: 2 },
              attribution: {
                kind: 'project',
                projectId: 1,
                package: { name: '@aviora/api', path: 'apps/api' },
              },
            }),
          ],
          error: null,
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    const services = await screen.findByRole('list', { name: /services/i });

    expect(within(services).getByText('apps/web')).toBeInTheDocument();
    expect(within(services).getByText('apps/api')).toBeInTheDocument();
  });

  it("shows only this project's services, not every listener on the machine", async () => {
    // A project view answers "what is running in *this* project". Every stray
    // listener on the machine — another app's helper, an editor's language
    // server — is noise there, and Mira positively determined those are not in
    // this project rather than failing to tell.
    backend(
      [project()],
      snapshot({
        services: {
          services: [
            service(),
            service({
              listener: { port: 7265, localAddress: '127.0.0.1', pid: 830 },
              attribution: { kind: 'unattributed', reason: 'outsideEveryProject' },
            }),
          ],
          error: null,
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    const services = await screen.findByRole('list', { name: /services/i });
    expect(within(services).getByText(':3000')).toBeInTheDocument();
    expect(within(services).queryByText(':7265')).not.toBeInTheDocument();
  });

  it('still shows a service it could not place, because that one might be yours', async () => {
    // The exception, and the reason it exists: on Windows Mira cannot read
    // another process's working directory at all. Hiding everything it could not
    // place would leave a Windows user looking at an empty list while their dev
    // server is plainly running.
    backend(
      [project()],
      snapshot({
        services: {
          services: [
            service({
              listener: { port: 5173, localAddress: '127.0.0.1', pid: 42 },
              attribution: { kind: 'unattributed', reason: 'noWorkingDirectory' },
            }),
          ],
          error: null,
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    const services = await screen.findByRole('list', { name: /services/i });
    expect(within(services).getByText(':5173')).toBeInTheDocument();
    expect(within(services).getByText(/does not let Mira see/i)).toBeInTheDocument();
  });

  it('says plainly when it cannot place a service', async () => {
    backend(
      [project()],
      snapshot({
        services: {
          services: [
            service({
              attribution: { kind: 'unattributed', reason: 'noOwningProcess' },
            }),
          ],
          error: null,
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    const services = await screen.findByRole('list', { name: /services/i });
    expect(within(services).getByText(/unattributed/i)).toBeInTheDocument();
    expect(within(services).getByText(/did not say which process/i)).toBeInTheDocument();
  });

  it("shows another project's service under that project, not this one", async () => {
    backend(
      [project({ id: 1, name: 'aviora' }), project({ id: 2, name: 'mobile', rootPath: '/b' })],
      snapshot({
        projects: [observation({ projectId: 1 }), observation({ projectId: 2 })],
        services: {
          services: [
            service({
              listener: { port: 4000, localAddress: '127.0.0.1', pid: 7 },
              attribution: { kind: 'project', projectId: 2, package: null },
            }),
          ],
          error: null,
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    await screen.findByRole('heading', { name: 'aviora' });
    expect(screen.queryByText(':4000')).not.toBeInTheDocument();
  });

  it('reports that port information is unavailable rather than showing none', async () => {
    backend(
      [project()],
      snapshot({
        services: {
          services: [],
          error: 'Reading the list of listening ports was refused.',
          observedAt: NOW,
        },
      }),
    );
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/port information unavailable/i)).toBeInTheDocument();
    expect(screen.getByText(/was refused/i)).toBeInTheDocument();
  });

  it('offers only safe actions', async () => {
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    await screen.findByRole('list', { name: /services/i });

    expect(screen.getByRole('button', { name: /^Open$/ })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /copy url/i })).toBeInTheDocument();
    // Stopping a service is a later slice with its own confirmation design.
    for (const destructive of [/kill/i, /stop/i, /restart/i, /terminate/i]) {
      expect(screen.queryByRole('button', { name: destructive })).not.toBeInTheDocument();
    }
  });

  it('opens a service by naming its port, never a URL', async () => {
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /^Open$/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('live_open_service', { port: 3000 }),
    );
  });
});

describe('freshness', () => {
  it('says when the reading was taken', async () => {
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    // Both the Git reading and the service list carry their own age; the
    // assertion is that a reading is dated, not that only one thing is.
    expect((await screen.findAllByText(/updated just now/i)).length).toBeGreaterThan(0);
  });

  it('shows the age of an older reading rather than implying it is current', async () => {
    vi.setSystemTime(new Date((NOW + 180) * 1000));
    backend([project()], snapshot());
    renderApp(<App surface="main" />);

    expect((await screen.findAllByText(/updated 3 min ago/i)).length).toBeGreaterThan(0);
  });

  it('reports a failed refresh beside the reading it could not replace', async () => {
    backend(
      [project()],
      snapshot({
        projects: [observation({ error: 'Git could not read this repository.' })],
      }),
    );
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/could not read this repository/i)).toBeInTheDocument();
    // The older reading stays visible: something true and old beats nothing.
    expect(screen.getByText('main')).toBeInTheDocument();
  });
});
