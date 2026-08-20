import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Project } from '../bindings/Project';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import { App } from '../App';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

function project(overrides: Partial<Project> = {}): Project {
  return {
    id: 1,
    name: 'Aviora',
    rootPath: '/home/dev/aviora',
    isGit: true,
    gitRoot: null,
    markers: ['node'],
    lastOpenedAt: 1_700_000_000,
    createdAt: 1_700_000_000,
    updatedAt: 1_700_000_000,
    ...overrides,
  };
}

const readyContext: LiveSnapshot = {
  projects: [
    {
      projectId: 1,
      directoryExists: true,
      git: {
        state: 'ready',
        head: { kind: 'branch', name: 'main' },
        clean: true,
        changed: 0,
        lastCommit: {
          sha: '9f2c1a4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b',
          shortSha: '9f2c1a4',
          subject: 'feat: improve workspace restoration',
          author: 'Blacknit',
          committedAt: 1_700_000_000,
        },
        upstream: { name: 'origin/main', ahead: 2, behind: 0 },
      },
      layout: { kind: 'standalone' },
      error: null,
      observedAt: 1_800_000_000,
    },
  ],
  services: { services: [], error: null, observedAt: 1_800_000_000 },
};

/** Route each command name to a canned answer. */
function backend(handlers: Record<string, unknown>) {
  invoke.mockImplementation((command: string) => {
    if (command === 'workspaces_list') return Promise.resolve([]);
    if (command === 'workspaces_applications') return Promise.resolve([]);
    if (!(command in handlers)) {
      return Promise.reject({ kind: 'notFound', what: command });
    }
    const answer = handlers[command];
    return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer);
  });
}

const shell = {
  projects_list: [],
  live_refresh: readyContext,
  live_snapshot: readyContext,
};

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('the project list', () => {
  it('offers one action when there is nothing yet', async () => {
    backend({ ...shell, projects_list: [] });
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/no projects yet/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /add project/i })).toBeInTheDocument();
  });

  it('lists every project the user has added', async () => {
    backend({
      ...shell,
      projects_list: [
        project({ id: 1, name: 'Aviora' }),
        project({ id: 2, name: 'Mobile App', rootPath: '/home/dev/mobile' }),
        project({ id: 3, name: 'Client API', rootPath: '/home/dev/api' }),
      ],
    });
    renderApp(<App surface="main" />);

    const list = await screen.findByRole('list', { name: /projects/i });
    const names = within(list)
      .getAllByRole('listitem')
      .map((item) => item.textContent);

    expect(names).toHaveLength(3);
    expect(names.join(' ')).toContain('Aviora');
    expect(names.join(' ')).toContain('Mobile App');
    expect(names.join(' ')).toContain('Client API');
  });

  it('shows a pending state while the list is being read', () => {
    invoke.mockImplementation(() => new Promise(() => {}));
    renderApp(<App surface="main" />);

    expect(screen.getByText(/reading/i)).toBeInTheDocument();
  });

  it('states what went wrong when the database cannot be read', async () => {
    backend({ projects_list: new Error('unused') });
    invoke.mockImplementation(() =>
      Promise.reject({ kind: 'external', source: 'SQLite', detail: 'disk I/O error' }),
    );
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/disk I\/O error/i)).toBeInTheDocument();
  });
});

describe('adding a project', () => {
  it('shows the new project without a reload', async () => {
    const added = project({ id: 7, name: 'Experiment', rootPath: '/home/dev/experiment' });
    let listed: Project[] = [];
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve(listed);
      if (command === 'projects_add') {
        listed = [added];
        return Promise.resolve(added);
      }
      return Promise.resolve(readyContext);
    });
    renderApp(<App surface="main" />);

    await screen.findByText(/no projects yet/i);
    await userEvent.click(screen.getByRole('button', { name: /add project/i }));

    const list = await screen.findByRole('list', { name: /projects/i });
    expect(within(list).getByText('Experiment')).toBeInTheDocument();
  });

  it('says nothing changed when the picker is dismissed', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([]);
      if (command === 'projects_add') return Promise.resolve(null);
      return Promise.resolve(readyContext);
    });
    renderApp(<App surface="main" />);

    await screen.findByText(/no projects yet/i);
    await userEvent.click(screen.getByRole('button', { name: /add project/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('projects_add', undefined));
    expect(screen.getByText(/no projects yet/i)).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('explains a directory that is already open', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([project()]);
      if (command === 'projects_add') {
        return Promise.reject({
          kind: 'invalid',
          field: 'path',
          detail: '/home/dev/aviora is already open as "Aviora".',
        });
      }
      return Promise.resolve(readyContext);
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('list', { name: /projects/i });
    await userEvent.click(screen.getByRole('button', { name: /add project/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/already open as "Aviora"/i);
  });

  it('clears the message once the person moves on', async () => {
    // A refusal that stays on screen after you have gone somewhere else stops
    // being information and becomes furniture.
    const second = project({ id: 2, name: 'Mobile App', rootPath: '/home/dev/mobile' });
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([project(), second]);
      if (command === 'projects_add') {
        return Promise.reject({
          kind: 'invalid',
          field: 'path',
          detail: '/home/dev/aviora is already open as "Aviora".',
        });
      }
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(readyContext);
      return Promise.resolve(second);
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('list', { name: /projects/i });
    await userEvent.click(screen.getByRole('button', { name: /add project/i }));
    expect(await screen.findByRole('alert')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /Mobile App/ }));

    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });

  it('never sends a path of its own', async () => {
    invoke.mockImplementation((command: string) =>
      Promise.resolve(command === 'projects_list' ? [] : null),
    );
    renderApp(<App surface="main" />);

    await screen.findByText(/no projects yet/i);
    await userEvent.click(screen.getByRole('button', { name: /add project/i }));

    await waitFor(() => {
      const call = invoke.mock.calls.find(([command]) => command === 'projects_add');
      expect(call).toBeDefined();
      // The directory is chosen by a native picker in Rust. The interface has no
      // path to send, and this asserts it never invents one.
      expect(call?.[1]).toBeUndefined();
    });
  });
});
