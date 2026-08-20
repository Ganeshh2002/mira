import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { GitOverview } from '../bindings/GitOverview';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import { App } from '../App';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const aviora: Project = {
  id: 1,
  name: 'Aviora',
  rootPath: '/home/dev/aviora',
  isGit: true,
  gitRoot: null,
  markers: ['node', 'rust'],
  lastOpenedAt: 1_700_000_000,
  createdAt: 1_700_000_000,
  updatedAt: 1_700_000_000,
};

const commit = {
  sha: '9f2c1a4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b',
  shortSha: '9f2c1a4',
  subject: 'feat: improve workspace restoration',
  author: 'Blacknit',
  committedAt: Math.floor(Date.now() / 1000) - 7200,
};

/** A snapshot in which the one project has this Git answer. */
function context(git: GitOverview | null, projectId = 1): LiveSnapshot {
  return {
    projects: [
      {
        projectId,
        directoryExists: git !== null,
        git,
        // Layout is the monorepo question and these tests are about Git; a
        // standalone repository is the shape that keeps them about one thing.
        layout: git === null ? null : { kind: 'standalone' },
        error: null,
        observedAt: 1_800_000_000,
      },
    ],
    services: { services: [], error: null, observedAt: 1_800_000_000 },
  };
}

/** Render the detail view for the one project, with this Git answer. */
function show(git: GitOverview | null) {
  invoke.mockImplementation((command: string) => {
    if (command === 'workspaces_list') return Promise.resolve([]);
    if (command === 'workspaces_applications') return Promise.resolve([]);
    if (command === 'projects_list') return Promise.resolve([aviora]);
    if (command === 'live_refresh' || command === 'live_snapshot')
      return Promise.resolve(context(git));
    return Promise.resolve(null);
  });
  return renderApp(<App surface="main" />);
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('the project overview', () => {
  it('shows the project and where it lives', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: commit,
      upstream: null,
    });

    expect(await screen.findByRole('heading', { name: 'Aviora' })).toBeInTheDocument();
    expect(screen.getByText('/home/dev/aviora')).toBeInTheDocument();
  });

  it('offers an action that opens the folder', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: null,
      upstream: null,
    });

    await userEvent.click(await screen.findByRole('button', { name: /open project/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith('projects_reveal', { projectId: 1 }),
    );
  });

  it('reads every project in one snapshot rather than one command each', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') {
        return Promise.resolve([aviora, { ...aviora, id: 2, name: 'Second', rootPath: '/b' }]);
      }
      return Promise.resolve(context({ state: 'notARepository' }));
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('heading', { name: 'Aviora' });

    // Observation is machine-wide and per-round, so there is no per-project Git
    // command left to call however many projects are on the list.
    const perProject = invoke.mock.calls.filter(([command]) => command === 'projects_context');
    expect(perProject).toHaveLength(0);
  });
});

describe('Git states', () => {
  it('reads clean when nothing has changed', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: commit,
      upstream: null,
    });

    expect(await screen.findByText(/clean/i)).toBeInTheDocument();
    expect(screen.getByText('main')).toBeInTheDocument();
  });

  it('counts the changes when the working tree is dirty', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: false,
      changed: 3,
      lastCommit: commit,
      upstream: null,
    });

    expect(await screen.findByText(/3 changed/i)).toBeInTheDocument();
  });

  it('shows the last commit with its author and age', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: commit,
      upstream: null,
    });

    expect(await screen.findByText('feat: improve workspace restoration')).toBeInTheDocument();
    expect(screen.getByText(/Blacknit/)).toBeInTheDocument();
    expect(screen.getByText(/2 h ago/)).toBeInTheDocument();
  });

  it('labels ahead and behind as of the last fetch', async () => {
    show({
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      clean: true,
      changed: 0,
      lastCommit: commit,
      upstream: { name: 'origin/main', ahead: 2, behind: 1 },
    });

    const ahead = await screen.findByText(/↑ 2 ahead/i);
    expect(ahead).toBeInTheDocument();
    expect(screen.getByText(/↓ 1 behind/i)).toBeInTheDocument();
    // FR-5.3: the counts are local and can be stale, and the interface says so
    // rather than implying it fetched.
    expect(screen.getByText(/as of last fetch/i)).toBeInTheDocument();
  });

  it('says a repository with no commits has none yet', async () => {
    show({
      state: 'ready',
      head: { kind: 'unborn' },
      clean: true,
      changed: 0,
      lastCommit: null,
      upstream: null,
    });

    expect(await screen.findByText(/no commits yet/i)).toBeInTheDocument();
  });

  it('marks a detached head as detached', async () => {
    show({
      state: 'ready',
      head: { kind: 'detached', sha: '9f2c1a4' },
      clean: true,
      changed: 0,
      // A different abbreviation from the head's, so the assertion below is about
      // the head and not about the commit row that happens to sit under it.
      lastCommit: { ...commit, shortSha: 'b71e0dd' },
      upstream: null,
    });

    expect(await screen.findByText(/detached/i)).toBeInTheDocument();
    expect(screen.getByText('9f2c1a4')).toBeInTheDocument();
  });

  it('treats a directory without Git as a state, not a failure', async () => {
    show({ state: 'notARepository' });

    expect(await screen.findByText(/not a repository/i)).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('reports a repository it could not read', async () => {
    show({ state: 'unreadable', detail: 'The HEAD reference could not be parsed.' });

    expect(await screen.findByText(/could not be parsed/i)).toBeInTheDocument();
  });

  it('says the folder is missing when the directory is gone', async () => {
    show(null);

    expect(await screen.findByText(/folder is missing/i)).toBeInTheDocument();
    // FR-1.5: never auto-deleted. The way out is offered, not taken.
    expect(screen.getByRole('button', { name: /remove project/i })).toBeInTheDocument();
  });

  it('shows a pending state while Git is being read', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora]);
      return new Promise(() => {});
    });
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/reading/i)).toBeInTheDocument();
  });

  it('reports a Git read that ran out of time', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora]);
      return Promise.reject({
        kind: 'timeout',
        operation: 'Reading Git for "Aviora"',
        afterMs: 2000,
      });
    });
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/did not finish within 2000 ms/i)).toBeInTheDocument();
  });
});

describe('refreshing', () => {
  it('re-reads Git when asked, so a change made outside Mira shows up', async () => {
    // FR-3.3 lists an explicit refresh among the triggers. The scheduler covers
    // the rest, but a person who has just saved a file should not have to wait
    // out an interval to confirm what they already know.
    let clean = true;
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora]);
      if (command === 'live_refresh' || command === 'live_snapshot') {
        const git: GitOverview = {
          state: 'ready',
          head: { kind: 'branch', name: 'main' },
          clean,
          changed: clean ? 0 : 2,
          lastCommit: commit,
          upstream: null,
        };
        clean = false;
        return Promise.resolve(context(git));
      }
      return Promise.resolve(null);
    });
    renderApp(<App surface="main" />);

    expect(await screen.findByText(/clean/i)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /refresh/i }));

    expect(await screen.findByText(/2 changed/i)).toBeInTheDocument();
  });
});

describe('switching projects', () => {
  it("leaves one project's error behind when another is selected", async () => {
    // An error about `aviora` shown while `mobile` is on screen is worse than no
    // error at all: it reports a problem with the wrong thing.
    const second: Project = {
      ...aviora,
      id: 2,
      name: 'Mobile App',
      rootPath: '/home/dev/mobile',
    };
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora, second]);
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(context({ state: 'notARepository' }));
      if (command === 'projects_reveal') {
        return Promise.reject({ kind: 'notFound', what: 'The folder for "Aviora"' });
      }
      return Promise.resolve(second);
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('heading', { name: 'Aviora' });
    await userEvent.click(screen.getByRole('button', { name: /open project/i }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/Aviora/);

    await userEvent.click(screen.getByRole('button', { name: /Mobile App/ }));

    await screen.findByRole('heading', { name: 'Mobile App' });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('forgets a half-finished removal when another project is selected', async () => {
    const second: Project = {
      ...aviora,
      id: 2,
      name: 'Mobile App',
      rootPath: '/home/dev/mobile',
    };
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora, second]);
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(context({ state: 'notARepository' }));
      return Promise.resolve(second);
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('heading', { name: 'Aviora' });
    await userEvent.click(screen.getByRole('button', { name: /remove project/i }));
    expect(
      screen.getByText(/Remove .Aviora. and its workspaces from Mira\?/),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /Mobile App/ }));

    await screen.findByRole('heading', { name: 'Mobile App' });
    expect(screen.queryByText(/from Mira\?/)).not.toBeInTheDocument();
  });

  it('re-reads when a project is selected again', async () => {
    // FR-3.3's first trigger. The scheduler would get there within five seconds
    // anyway; five seconds is long enough to doubt what you are looking at.
    const second: Project = {
      ...aviora,
      id: 2,
      name: 'Mobile App',
      rootPath: '/home/dev/mobile',
    };
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora, second]);
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(context({ state: 'notARepository' }));
      return Promise.resolve(second);
    });
    renderApp(<App surface="main" />);

    await screen.findByRole('heading', { name: 'Aviora' });
    const reads = () =>
      invoke.mock.calls.filter(
        ([command]) => command === 'live_refresh' || command === 'live_snapshot',
      ).length;
    const before = reads();

    await userEvent.click(screen.getByRole('button', { name: /Mobile App/ }));

    await waitFor(() => expect(reads()).toBeGreaterThan(before));
  });

  it('reads the second project without disturbing the first', async () => {
    const second: Project = {
      ...aviora,
      id: 2,
      name: 'Mobile App',
      rootPath: '/home/dev/mobile',
    };
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora, second]);
      if (command === 'projects_open') return Promise.resolve(second);
      if (command === 'live_refresh' || command === 'live_snapshot') {
        return Promise.resolve({
          projects: [
            {
              projectId: 1,
              directoryExists: true,
              git: {
                state: 'ready',
                head: { kind: 'branch', name: 'main' },
                clean: true,
                changed: 0,
                lastCommit: commit,
                upstream: null,
              },
              layout: { kind: 'standalone' },
              error: null,
              observedAt: 1_800_000_000,
            },
            {
              projectId: 2,
              directoryExists: true,
              git: {
                state: 'ready',
                head: { kind: 'branch', name: 'develop' },
                clean: false,
                changed: 5,
                lastCommit: null,
                upstream: null,
              },
              layout: { kind: 'standalone' },
              error: null,
              observedAt: 1_800_000_000,
            },
          ],
          services: { services: [], error: null, observedAt: 1_800_000_000 },
        });
      }
      return Promise.resolve(null);
    });
    renderApp(<App surface="main" />);

    expect(await screen.findByText('main')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /Mobile App/ }));

    expect(await screen.findByText('develop')).toBeInTheDocument();
    expect(screen.getByText(/5 changed/i)).toBeInTheDocument();
    expect(screen.queryByText('main')).not.toBeInTheDocument();
  });

  it('records that the project was opened', async () => {
    const second: Project = {
      ...aviora,
      id: 2,
      name: 'Mobile App',
      rootPath: '/home/dev/mobile',
    };
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora, second]);
      if (command === 'live_refresh' || command === 'live_snapshot')
        return Promise.resolve(context({ state: 'notARepository' }));
      return Promise.resolve(second);
    });
    renderApp(<App surface="main" />);

    await userEvent.click(await screen.findByRole('button', { name: /Mobile App/ }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('projects_open', { projectId: 2 }));
  });
});
