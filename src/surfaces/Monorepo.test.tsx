import { screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { RepositoryLayout } from '../bindings/RepositoryLayout';
import { App } from '../App';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const aviora: Project = {
  id: 1,
  name: 'aviora',
  rootPath: '/home/dev/aviora',
  isGit: true,
  gitRoot: null,
  markers: ['node'],
  lastOpenedAt: 1_700_000_000,
  createdAt: 1_700_000_000,
  updatedAt: 1_700_000_000,
};

const clean: LiveSnapshot['projects'][number]['git'] = {
  state: 'ready',
  head: { kind: 'branch', name: 'main' },
  clean: true,
  changed: 0,
  lastCommit: null,
  upstream: null,
};

function show(layout: RepositoryLayout, project: Project = aviora) {
  invoke.mockImplementation((command: string) => {
    if (command === 'workspaces_list') return Promise.resolve([]);
    if (command === 'workspaces_applications') return Promise.resolve([]);
    if (command === 'projects_list') return Promise.resolve([project]);
    if (command === 'live_refresh' || command === 'live_snapshot') {
      return Promise.resolve({
        projects: [
          {
            projectId: project.id,
            directoryExists: true,
            git: clean,
            layout,
            error: null,
            observedAt: 1_800_000_000,
          },
        ],
        services: { services: [], error: null, observedAt: 1_800_000_000 },
      });
    }
    return Promise.resolve(null);
  });
  return renderApp(<App surface="main" />);
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe('a standalone repository', () => {
  it('says nothing about monorepos', async () => {
    show({ kind: 'standalone' });

    await screen.findByRole('heading', { name: 'aviora' });
    expect(screen.queryByText(/monorepo/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /packages/i })).not.toBeInTheDocument();
  });
});

describe('a monorepo root', () => {
  const layout: RepositoryLayout = {
    kind: 'monorepoRoot',
    tools: ['pnpmWorkspaces', 'turborepo'],
    packages: [
      { name: '@aviora/web', path: 'apps/web' },
      { name: '@aviora/mobile', path: 'apps/mobile' },
      { name: '@aviora/api', path: 'packages/api' },
      { name: '@aviora/ui', path: 'packages/ui' },
    ],
  };

  it('names the tools that declare it', async () => {
    show(layout);

    expect(await screen.findByText(/pnpm workspaces/i)).toBeInTheDocument();
    expect(screen.getByText(/turborepo/i)).toBeInTheDocument();
  });

  it('lists every package it found', async () => {
    show(layout);

    const packages = await screen.findByRole('list', { name: /packages/i });
    const rows = within(packages).getAllByRole('listitem');

    expect(rows).toHaveLength(4);
    expect(rows.map((row) => row.textContent).join(' ')).toContain('apps/web');
    expect(rows.map((row) => row.textContent).join(' ')).toContain('packages/ui');
  });

  it('shows each package where it actually is', async () => {
    show(layout);

    const packages = await screen.findByRole('list', { name: /packages/i });
    expect(within(packages).getByText('apps/mobile')).toBeInTheDocument();
  });

  it('still shows the repository-wide Git state', async () => {
    show(layout);

    expect(await screen.findByText('main')).toBeInTheDocument();
    expect(screen.getByText(/clean/i)).toBeInTheDocument();
  });

  it('renders no package list when there are no packages to list', async () => {
    show({ kind: 'monorepoRoot', tools: ['nx'], packages: [] });

    await screen.findByRole('heading', { name: 'aviora' });
    expect(screen.queryByRole('list', { name: /packages/i })).not.toBeInTheDocument();
  });
});

describe('a package inside a monorepo', () => {
  const web: Project = {
    ...aviora,
    id: 2,
    name: 'web',
    rootPath: '/home/dev/aviora/apps/web',
    gitRoot: '/home/dev/aviora',
  };

  const layout: RepositoryLayout = {
    kind: 'package',
    tools: ['pnpmWorkspaces'],
    monorepoRoot: '/home/dev/aviora',
    packagePath: 'apps/web',
    packageName: '@aviora/web',
  };

  it('shows the repository it belongs to and the package it is', async () => {
    show(layout, web);

    expect(await screen.findByText('/home/dev/aviora')).toBeInTheDocument();
    expect(screen.getByText('apps/web')).toBeInTheDocument();
  });

  it('names the package by what its own manifest calls it', async () => {
    show(layout, web);

    expect(await screen.findByText('@aviora/web')).toBeInTheDocument();
  });

  it('does not pretend the package is its own repository', async () => {
    show(layout, web);

    await screen.findByText('apps/web');
    // The Git state shown is the repository's, and the header says which
    // repository that is. Nothing here implies `apps/web` has a Git of its own.
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: /packages/i })).not.toBeInTheDocument();
  });

  it('is observed by the same round as every other project', async () => {
    show(layout, web);

    await screen.findByText('apps/web');

    // One machine-wide observation covers every project, package or not. There
    // is no per-project command, so a monorepo package costs nothing extra.
    const perProject = invoke.mock.calls.filter(([command]) => command === 'projects_context');
    expect(perProject).toHaveLength(0);
    expect(
      invoke.mock.calls.filter(([command]) => command === 'live_refresh').length,
    ).toBeGreaterThan(0);
  });
});

describe('a project whose folder is gone', () => {
  it('says nothing about layout it could not read', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_list') return Promise.resolve([]);
      if (command === 'workspaces_applications') return Promise.resolve([]);
      if (command === 'projects_list') return Promise.resolve([aviora]);
      if (command === 'live_refresh' || command === 'live_snapshot') {
        return Promise.resolve({
          projects: [
            {
              projectId: aviora.id,
              directoryExists: false,
              git: null,
              layout: null,
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

    expect(await screen.findByText(/folder is missing/i)).toBeInTheDocument();
    expect(screen.queryByText(/monorepo/i)).not.toBeInTheDocument();
  });
});
