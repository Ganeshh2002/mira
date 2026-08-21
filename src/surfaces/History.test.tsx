import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { CommitGraph } from '../bindings/CommitGraph';
import type { CommitLookup } from '../bindings/CommitLookup';
import type { CommitPage } from '../bindings/CommitPage';
import type { ChangedFiles } from '../bindings/ChangedFiles';
import type { FilteredHistory } from '../bindings/FilteredHistory';
import type { KnownAuthors } from '../bindings/KnownAuthors';
import type { KnownRefs } from '../bindings/KnownRefs';
import type { GraphRow } from '../bindings/GraphRow';
import type { Project } from '../bindings/Project';
import type { RepositoryLayout } from '../bindings/RepositoryLayout';
import { History } from './History';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const NOW = 1_800_000_000;

function project(overrides: Partial<Project> = {}): Project {
  return {
    id: 1,
    name: 'aviora',
    rootPath: '/home/dev/aviora',
    isGit: true,
    gitRoot: null,
    markers: [],
    lastOpenedAt: NOW,
    createdAt: NOW,
    updatedAt: NOW,
    ...overrides,
  };
}

/** `count` commits, newest first, with ids that are recognisably distinct. */
function commits(count: number, from = 0) {
  return Array.from({ length: count }, (_, n) => {
    const index = from + n;
    const sha = `${index}`.padStart(2, 'a').repeat(20).slice(0, 40);
    return {
      sha,
      shortSha: sha.slice(0, 7),
      subject: `commit ${index}`,
      author: 'Ganeshh',
      committedAt: NOW - index * 3600,
    };
  });
}

function page(overrides: Partial<Extract<CommitPage, { state: 'ready' }>> = {}): CommitPage {
  return {
    state: 'ready',
    head: { kind: 'branch', name: 'main' },
    commits: commits(3),
    next: null,
    shallow: false,
    ...overrides,
  };
}

/**
 * The graph page a history page corresponds to.
 *
 * The two commands answer the same question with different detail, so the
 * fixtures below describe the history once and this derives the graph — which is
 * also the property the surface relies on: switching modes must not change which
 * commits are shown.
 */
function asGraph(page: CommitPage, shape: Partial<GraphRow>[] = []): CommitGraph {
  if (page.state !== 'ready') return page;

  return {
    state: 'ready',
    head: page.head,
    next: page.next,
    shallow: page.shallow,
    lanes: 1,
    collapsed: false,
    refsTruncated: false,
    rows: page.commits.map((commit, index) => ({
      commit,
      parents: [],
      lane: 0,
      kind: 'normal',
      refs: [],
      edges: [],
      continuing: [],
      ...shape[index],
    })),
  };
}

/**
 * Serve one or more pages, in order, to whichever command asks.
 *
 * Both `git_history` and `git_graph` are answered from the same fixtures, so a
 * test says what the history is and does not have to care which mode the surface
 * happens to be in.
 */
function backend(
  pages: CommitPage[],
  extras: Record<string, unknown> = {},
  shape: Partial<GraphRow>[] = [],
  graphOverride?: Partial<Extract<CommitGraph, { state: 'ready' }>>,
) {
  let served = 0;
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    if (command === 'git_history' || command === 'git_graph') {
      const next = pages[Math.min(served, pages.length - 1)]!;
      served += 1;
      if (command === 'git_history') return Promise.resolve(next);
      const graph = asGraph(next, shape);
      return Promise.resolve(
        graph.state === 'ready' && graphOverride ? { ...graph, ...graphOverride } : graph,
      );
    }
    if (command in extras) return Promise.resolve(extras[command]);
    if (command === 'git_copy_commit') {
      const commit = (args as { commit?: string } | undefined)?.commit;
      return Promise.resolve(commit ?? '');
    }
    // The filter bar's three menus. Empty by default, so a test that is not
    // about filtering does not have to say anything about it.
    if (command === 'git_refs') return Promise.resolve(NO_REFS);
    if (command === 'git_authors') return Promise.resolve(NO_AUTHORS);
    if (command === 'git_changes') return Promise.resolve(NO_CHANGES);
    return Promise.resolve(undefined);
  });
}

const NO_REFS: KnownRefs = { state: 'ready', refs: [], truncated: false };
const NO_AUTHORS: KnownAuthors = {
  state: 'ready',
  authors: [],
  scanned: 0,
  stopped: { state: 'no' },
};
const NO_CHANGES: ChangedFiles = {
  state: 'ready',
  files: [],
  against: { kind: 'parent' },
  truncated: { state: 'no' },
};

function show(layout: RepositoryLayout | null = null, onBack = vi.fn()) {
  return renderApp(<History project={project()} layout={layout} onBack={onBack} />);
}

beforeEach(() => {
  invoke.mockReset();
});

describe('the history surface', () => {
  it('lists commits newest first, as the repository ordered them', async () => {
    backend([page()]);
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const rows = within(list).getAllByRole('listitem');

    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent('commit 0');
    expect(rows[2]).toHaveTextContent('commit 2');
  });

  it('says who wrote each commit, when, and its short id — without hover', async () => {
    backend([page({ commits: commits(1) })]);
    show();

    const row = within(await screen.findByRole('list', { name: 'Commits' })).getByRole(
      'listitem',
    );

    expect(row).toHaveTextContent('commit 0');
    expect(row).toHaveTextContent('Ganeshh');
    expect(row).toHaveTextContent(/ago|just now/);
    expect(row).toHaveTextContent(commits(1)[0]!.shortSha);
  });

  it('names the branch the history is being read from', async () => {
    backend([page()]);
    show();

    expect(await screen.findByText('main')).toBeInTheDocument();
  });

  it('says where HEAD is when it is not on a branch', async () => {
    backend([page({ head: { kind: 'detached', sha: 'ad50fc7' } })]);
    show();

    expect(await screen.findByText(/Detached HEAD · ad50fc7/)).toBeInTheDocument();
  });

  it('names the repository a package belongs to, not the package', async () => {
    // History belongs to the repository. Two packages in one monorepo see the
    // same history, and this line is what says whose it is.
    backend([page()]);
    show({
      kind: 'package',
      tools: ['pnpmWorkspaces'],
      monorepoRoot: '/home/dev/aviora',
      packagePath: 'apps/web',
      packageName: '@aviora/web',
    });

    expect(await screen.findByText(/Repository · aviora/)).toBeInTheDocument();
  });
});

describe('paging', () => {
  it('never asks for more than a page, and never asks for a size', async () => {
    backend([page({ commits: commits(25), next: 'b'.repeat(40) })]);
    show();

    await screen.findByRole('list', { name: 'Commits' });

    const asked = invoke.mock.calls.find(
      ([command]) => command === 'git_graph' || command === 'git_history',
    );
    const sent = (asked?.[1] ?? {}) as Record<string, unknown>;
    expect(sent).toEqual({ projectId: 1, cursor: null });
    // There is no page-size argument to send, and a guard test in Rust fails the
    // build if a command ever grows one.
    expect(Object.keys(sent)).not.toContain('limit');
  });

  it('offers Load more only while there is more, and continues from the cursor', async () => {
    const cursor = 'b'.repeat(40);
    backend([
      page({ commits: commits(3), next: cursor }),
      page({ commits: commits(2, 3), next: null }),
    ]);
    show();

    const more = await screen.findByRole('button', { name: 'Load more' });
    await userEvent.click(more);

    await waitFor(() => {
      expect(
        within(screen.getByRole('list', { name: 'Commits' })).getAllByRole('listitem'),
      ).toHaveLength(5);
    });

    const second = invoke.mock.calls.filter(
      ([command]) => command === 'git_graph' || command === 'git_history',
    )[1];
    expect(second?.[1]).toEqual({ projectId: 1, cursor });
    expect(screen.queryByRole('button', { name: 'Load more' })).not.toBeInTheDocument();
  });

  it('does not offer Load more when the whole history fits on one page', async () => {
    backend([page()]);
    show();

    await screen.findByRole('list', { name: 'Commits' });
    expect(screen.queryByRole('button', { name: 'Load more' })).not.toBeInTheDocument();
  });

  it('reads only when asked — never on a clock', async () => {
    backend([page()]);
    show();

    await screen.findByRole('list', { name: 'Commits' });
    const reads = () =>
      invoke.mock.calls.filter(
        ([command]) => command === 'git_graph' || command === 'git_history',
      ).length;
    const before = reads();

    await userEvent.click(screen.getByRole('button', { name: 'Refresh' }));

    await waitFor(() => expect(reads()).toBe(before + 1));
  });
});

describe('the states that are not the happy path', () => {
  it('says a repository with no commits has none, rather than showing nothing', async () => {
    backend([page({ head: { kind: 'unborn' }, commits: [] })]);
    show();

    expect(await screen.findByText('No commits yet')).toBeInTheDocument();
  });

  it('says a directory is not a repository rather than failing', async () => {
    backend([{ state: 'notARepository' }]);
    show();

    expect(await screen.findByText('Not a repository')).toBeInTheDocument();
  });

  it('shows the reason a repository could not be read', async () => {
    backend([{ state: 'unreadable', detail: 'HEAD does not name a reference.' }]);
    show();

    expect(await screen.findByText('History cannot be read')).toBeInTheDocument();
    expect(screen.getByText('HEAD does not name a reference.')).toBeInTheDocument();
  });

  it('says a shallow copy is shallow, so the oldest row is not the beginning', async () => {
    backend([page({ shallow: true })]);
    show();

    expect(await screen.findByText(/shallow copy/)).toBeInTheDocument();
  });
});

describe('a commit', () => {
  const detail: CommitLookup = {
    state: 'ready',
    commit: {
      commit: commits(1)[0]!,
      body: 'A longer explanation.',
      authorEmail: 'ganeshh@example.com',
      parents: 1,
      changedFiles: 4,
    },
  };

  it('opens its detail when the row is clicked', async () => {
    backend([page()], { git_commit: detail });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);

    expect(await screen.findByText('A longer explanation.')).toBeInTheDocument();
    expect(screen.getByText(commits(1)[0]!.sha)).toBeInTheDocument();
    expect(screen.getByText(/ganeshh@example.com/)).toBeInTheDocument();
    expect(screen.getByText('4 paths')).toBeInTheDocument();
  });

  it('says a merge is a merge and does not invent a file count', async () => {
    backend([page()], {
      git_commit: {
        state: 'ready',
        commit: {
          commit: commits(1)[0]!,
          body: null,
          authorEmail: 'ganeshh@example.com',
          parents: 2,
          changedFiles: null,
        },
      } satisfies CommitLookup,
    });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);

    expect(await screen.findByText('2 — a merge')).toBeInTheDocument();
    expect(screen.getByText('Not counted')).toBeInTheDocument();
  });

  it('says a commit is not here rather than failing', async () => {
    backend([page()], { git_commit: { state: 'unknown' } satisfies CommitLookup });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);

    expect(await screen.findByText('Not in this repository')).toBeInTheDocument();
  });

  it('offers nothing that would change the repository', async () => {
    backend([page()], { git_commit: detail });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);
    await screen.findByText('A longer explanation.');

    // Read-only, and visibly so: not disabled, absent. A greyed-out Revert would
    // promise a later release (`information-architecture.md` §5).
    for (const write of [
      /check ?out/i,
      /revert/i,
      /reset/i,
      /cherry.?pick/i,
      /rebase/i,
      /push/i,
    ]) {
      expect(screen.queryByRole('button', { name: write })).not.toBeInTheDocument();
    }
  });
});

describe('copying a commit id', () => {
  it('names the commit, never the text, and says which spelling', async () => {
    backend([page({ commits: commits(1) })]);
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getByRole('button', { name: 'Copy short SHA' }));

    await waitFor(() => {
      const copied = invoke.mock.calls.find(([command]) => command === 'git_copy_commit');
      expect(copied?.[1]).toEqual({
        projectId: 1,
        commit: commits(1)[0]!.sha,
        form: 'short',
      });
    });
  });

  it('offers the full id from the commit detail', async () => {
    backend([page({ commits: commits(1) })], {
      git_commit: {
        state: 'ready',
        commit: {
          commit: commits(1)[0]!,
          body: null,
          authorEmail: 'ganeshh@example.com',
          parents: 1,
          changedFiles: 1,
        },
      } satisfies CommitLookup,
    });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);

    const full = await screen.findByRole('button', { name: 'Copy full SHA' });
    await userEvent.click(full);

    await waitFor(() => {
      const copied = invoke.mock.calls.find(
        ([command, args]) =>
          command === 'git_copy_commit' &&
          (args as { form?: string } | undefined)?.form === 'full',
      );
      expect(copied).toBeDefined();
    });
  });

  it('confirms what happened, in a word', async () => {
    backend([page({ commits: commits(1) })]);
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getByRole('button', { name: 'Copy short SHA' }));

    expect(await screen.findByRole('status')).toHaveTextContent('Copied');
  });

  it('says why nothing was copied when the platform refused', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'git_history') return Promise.resolve(page({ commits: commits(1) }));
      if (command === 'git_graph') {
        return Promise.resolve(asGraph(page({ commits: commits(1) })));
      }
      if (command === 'git_copy_commit') {
        return Promise.reject({
          kind: 'unsupported',
          capability: 'clipboard',
          reason: 'This machine has no clipboard Mira can write to',
        });
      }
      return Promise.resolve(undefined);
    });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getByRole('button', { name: 'Copy short SHA' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/no clipboard/);
  });
});

describe('leaving', () => {
  it('goes back to the project', async () => {
    const onBack = vi.fn();
    backend([page()]);
    show(null, onBack);

    await userEvent.click(await screen.findByRole('button', { name: 'Back to aviora' }));

    expect(onBack).toHaveBeenCalled();
  });

  it('goes back one level on Escape', async () => {
    const onBack = vi.fn();
    backend([page()]);
    show(null, onBack);

    await screen.findByRole('list', { name: 'Commits' });
    await userEvent.keyboard('{Escape}');

    expect(onBack).toHaveBeenCalled();
  });

  it('closes an open commit before leaving the surface', async () => {
    const onBack = vi.fn();
    backend([page()], {
      git_commit: {
        state: 'ready',
        commit: {
          commit: commits(1)[0]!,
          body: 'A longer explanation.',
          authorEmail: 'ganeshh@example.com',
          parents: 1,
          changedFiles: 1,
        },
      } satisfies CommitLookup,
    });
    show(null, onBack);

    const list = await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(within(list).getAllByRole('button')[0]!);
    await screen.findByText('A longer explanation.');

    await userEvent.keyboard('{Escape}');

    await waitFor(() => {
      expect(screen.queryByText('A longer explanation.')).not.toBeInTheDocument();
    });
    expect(onBack).not.toHaveBeenCalled();
  });
});

describe('the graph', () => {
  /** A branch merged back into a mainline, as the backend would send it. */
  function merged(): CommitGraph {
    const four = commits(4);
    return {
      state: 'ready',
      head: { kind: 'branch', name: 'main' },
      next: null,
      shallow: false,
      lanes: 2,
      collapsed: false,
      refsTruncated: false,
      rows: [
        {
          commit: four[0]!,
          parents: [four[1]!.sha, four[2]!.sha],
          lane: 0,
          kind: 'merge',
          refs: [
            { kind: 'head', name: 'HEAD' },
            { kind: 'branch', name: 'main' },
          ],
          edges: [
            { to: 0, kind: 'straight' },
            { to: 1, kind: 'merge' },
          ],
          continuing: [0, 1],
        },
        {
          commit: four[1]!,
          parents: [four[3]!.sha],
          lane: 0,
          kind: 'normal',
          refs: [],
          edges: [{ to: 0, kind: 'straight' }],
          continuing: [0, 1],
        },
        {
          commit: four[2]!,
          parents: [four[3]!.sha],
          lane: 1,
          kind: 'normal',
          refs: [{ kind: 'tag', name: 'v1.0' }],
          edges: [{ to: 0, kind: 'join' }],
          continuing: [0],
        },
        {
          commit: four[3]!,
          parents: [],
          lane: 0,
          kind: 'root',
          refs: [],
          edges: [],
          continuing: [],
        },
      ],
    };
  }

  /** Serve one graph page, whichever command is asked. */
  function graphBackend(graph: CommitGraph) {
    invoke.mockImplementation((command: string) => {
      if (command === 'git_graph') return Promise.resolve(graph);
      if (command === 'git_history') return Promise.resolve(page());
      return Promise.resolve(undefined);
    });
  }

  it('draws the graph by default', async () => {
    graphBackend(merged());
    show();

    await screen.findByRole('list', { name: 'Commits' });
    expect(screen.getByRole('button', { name: 'Graph' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(invoke.mock.calls.some(([command]) => command === 'git_graph')).toBe(true);
  });

  it('reads the cheaper command when the graph is turned off', async () => {
    // The two modes are a difference in cost, not a second surface: List asks
    // for the commits alone, without parents, lanes or reference labels.
    graphBackend(merged());
    show();

    await screen.findByRole('list', { name: 'Commits' });
    await userEvent.click(screen.getByRole('button', { name: 'Graph' }));

    await waitFor(() => {
      expect(invoke.mock.calls.some(([command]) => command === 'git_history')).toBe(true);
    });
    expect(screen.getByRole('button', { name: 'Graph' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });

  it('says a merge is a merge in words, not only in the picture', async () => {
    // `design-system.md` §5: colour and shape always have a text partner. With
    // the gutter hidden — a narrow window, a screen reader — the row still says
    // what kind of commit it is.
    graphBackend(merged());
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const rows = within(list).getAllByRole('listitem');

    expect(rows[0]).toHaveTextContent('Merge of 2 parents');
    expect(rows[3]).toHaveTextContent('First commit');
    expect(rows[1]).not.toHaveTextContent('Merge');
  });

  it('labels the rows a branch or a tag points at', async () => {
    graphBackend(merged());
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const rows = within(list).getAllByRole('listitem');

    expect(within(rows[0]!).getByTitle('Branch: main')).toBeInTheDocument();
    expect(within(rows[0]!).getByTitle('Where HEAD is: HEAD')).toBeInTheDocument();
    expect(within(rows[2]!).getByTitle('Tag: v1.0')).toBeInTheDocument();
  });

  it('keeps the picture out of the accessibility tree', async () => {
    // Everything the gutter draws is written on the row beside it, so announcing
    // it would be reading out a picture rather than the thing it pictures.
    graphBackend(merged());
    const { container } = show();

    await screen.findByRole('list', { name: 'Commits' });
    const drawings = container.querySelectorAll('svg[aria-hidden="true"]');

    expect(drawings.length).toBeGreaterThan(0);
    expect(container.querySelectorAll('svg[role="img"]')).toHaveLength(0);
  });

  it('says when more branches meet than it can draw', async () => {
    graphBackend({ ...merged(), state: 'ready', lanes: 8, collapsed: true } as CommitGraph);
    show();

    expect(await screen.findByText(/past the eighth lane/)).toBeInTheDocument();
    expect(screen.getByText(/Every commit is still listed/)).toBeInTheDocument();
  });

  it('says when a label may be missing', async () => {
    graphBackend({ ...merged(), state: 'ready', refsTruncated: true } as CommitGraph);
    show();

    expect(
      await screen.findByText(/more references than Mira reads at once/),
    ).toBeInTheDocument();
  });

  it('offers nothing that would change the repository', async () => {
    // The graph is a picture, not a client — and the absence is visible rather
    // than a set of greyed-out controls promising a later release.
    graphBackend(merged());
    show();

    await screen.findByRole('list', { name: 'Commits' });

    for (const write of [
      /check ?out/i,
      /^merge$/i,
      /rebase/i,
      /reset/i,
      /cherry.?pick/i,
      /revert/i,
      /^push$/i,
      /^pull$/i,
      /fetch/i,
      /stage/i,
      /new branch/i,
      /delete branch/i,
    ]) {
      expect(screen.queryByRole('button', { name: write })).not.toBeInTheDocument();
    }
  });

  it('moves between commits with the arrow keys', async () => {
    // `information-architecture.md` §7: every list moves with the arrows, and
    // every destination stays reachable by keyboard.
    graphBackend(merged());
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const rows = within(list)
      .getAllByRole('button')
      .filter((button) => button.hasAttribute('data-commit'));

    rows[0]!.focus();
    expect(rows[0]).toHaveFocus();

    await userEvent.keyboard('{ArrowDown}');
    expect(rows[1]).toHaveFocus();

    await userEvent.keyboard('{ArrowDown}{ArrowUp}');
    expect(rows[1]).toHaveFocus();
  });

  it('does not wander off either end of the list', async () => {
    graphBackend(merged());
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const rows = within(list)
      .getAllByRole('button')
      .filter((button) => button.hasAttribute('data-commit'));

    rows[0]!.focus();
    await userEvent.keyboard('{ArrowUp}');
    expect(rows[0]).toHaveFocus();

    rows[rows.length - 1]!.focus();
    await userEvent.keyboard('{ArrowDown}');
    expect(rows[rows.length - 1]).toHaveFocus();
  });

  it('opens a commit from the keyboard', async () => {
    graphBackend({
      ...merged(),
      state: 'ready',
    } as CommitGraph);
    invoke.mockImplementation((command: string) => {
      if (command === 'git_graph') return Promise.resolve(merged());
      if (command === 'git_commit') {
        return Promise.resolve({
          state: 'ready',
          commit: {
            commit: commits(1)[0]!,
            body: 'A longer explanation.',
            authorEmail: 'ganeshh@example.com',
            parents: 2,
            changedFiles: null,
          },
        } satisfies CommitLookup);
      }
      return Promise.resolve(undefined);
    });
    show();

    const list = await screen.findByRole('list', { name: 'Commits' });
    const first = within(list)
      .getAllByRole('button')
      .find((button) => button.hasAttribute('data-commit'))!;

    first.focus();
    await userEvent.keyboard('{Enter}');

    expect(await screen.findByText('A longer explanation.')).toBeInTheDocument();
  });
});

// ── Narrowing ────────────────────────────────────────────────────────────────

const REFS: KnownRefs = {
  state: 'ready',
  refs: [
    { kind: 'branch', name: 'main', tip: 'b'.repeat(40) },
    { kind: 'branch', name: 'feature/graph', tip: 'c'.repeat(40) },
    { kind: 'tag', name: 'v1.0', tip: 'd'.repeat(40) },
  ],
  truncated: false,
};

const AUTHORS: KnownAuthors = {
  state: 'ready',
  authors: [
    { name: 'Ganeshh', commits: 40 },
    { name: 'Grace Hopper', commits: 2 },
  ],
  scanned: 42,
  stopped: { state: 'no' },
};

const CHANGES: ChangedFiles = {
  state: 'ready',
  files: [
    {
      at: 0,
      kind: 'modified',
      path: 'src/app.ts',
      fromPath: null,
      binary: false,
      additions: 4,
      deletions: 1,
    },
    {
      at: 1,
      kind: 'added',
      path: 'docs/notes.md',
      fromPath: null,
      binary: false,
      additions: 9,
      deletions: 0,
    },
  ],
  against: { kind: 'head' },
  truncated: { state: 'no' },
};

function found(
  overrides: Partial<Extract<FilteredHistory, { state: 'ready' }>> = {},
): FilteredHistory {
  return {
    state: 'ready',
    head: { kind: 'branch', name: 'main' },
    commits: commits(2),
    next: null,
    scanned: 120,
    stopped: { state: 'no' },
    shallow: false,
    ...overrides,
  };
}

/** The full history, plus menus with something in them and a search to serve. */
function searchable(results: FilteredHistory[] = [found()]) {
  let served = 0;
  backend([page()], {
    git_refs: REFS,
    git_authors: AUTHORS,
    git_changes: CHANGES,
  });

  const paged = invoke.getMockImplementation() as (
    command: string,
    args?: Record<string, unknown>,
  ) => Promise<unknown>;
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    if (command === 'git_search') {
      const next = results[Math.min(served, results.length - 1)]!;
      served += 1;
      return Promise.resolve(next);
    }
    return paged(command, args);
  });
}

/** Open one of the bar's menus and pick something from it. */
async function pick(menu: string, option: string) {
  await userEvent.click(await screen.findByRole('button', { name: new RegExp(`^${menu}`) }));
  await userEvent.click(await screen.findByRole('menuitem', { name: new RegExp(option) }));
}

/** The arguments of the last search, whatever they were. */
function lastSearch() {
  const calls = invoke.mock.calls.filter(([command]) => command === 'git_search');
  return calls[calls.length - 1]?.[1] as
    { projectId: number; wanted: Record<string, unknown>; cursor: string | null } | undefined;
}

describe('the filter bar', () => {
  it('does not narrow anything until something is chosen', async () => {
    searchable();
    show();

    await screen.findByRole('list', { name: 'Commits' });

    expect(screen.getByRole('search', { name: 'Filter history' })).toBeInTheDocument();
    expect(invoke.mock.calls.some(([command]) => command === 'git_search')).toBe(false);
  });

  it('sends a branch as its tip and never as its name', async () => {
    // The contract the slice rests on: `main` is a label, `bbbb…` is the question.
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Branch', 'main');

    await waitFor(() => expect(lastSearch()).toBeDefined());
    expect(lastSearch()?.wanted).toEqual({
      branch: 'b'.repeat(40),
      author: null,
      subject: null,
      file: null,
    });
    expect(JSON.stringify(lastSearch())).not.toContain('main');
    expect(JSON.stringify(lastSearch())).not.toContain('refs/');
  });

  it('sends a file as its place in a change list and never as a path', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('File', 'src/app.ts');

    await waitFor(() => expect(lastSearch()).toBeDefined());
    expect(lastSearch()?.wanted.file).toEqual({
      scope: { kind: 'workingTree' },
      at: 0,
      before: false,
    });
    expect(JSON.stringify(lastSearch())).not.toContain('src/app.ts');
  });

  it('searches subject text only when it is submitted', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    const field = screen.getByRole('searchbox', { name: 'Search' });
    await userEvent.type(field, 'widget');

    expect(invoke.mock.calls.some(([command]) => command === 'git_search')).toBe(false);

    await userEvent.type(field, '{Enter}');

    await waitFor(() => expect(lastSearch()?.wanted.subject).toBe('widget'));
  });

  it('says what the search does, since “search” means five things', async () => {
    searchable();
    show();

    expect(
      await screen.findByRole('searchbox', { name: 'Search' }),
    ).toHaveAccessibleDescription(
      /Matches part of a commit’s subject line, ignoring case\. Not a pattern/,
    );
  });

  it('composes filters rather than replacing them', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Branch', 'main');
    await pick('Author', 'Grace Hopper');
    await userEvent.type(screen.getByRole('searchbox', { name: 'Search' }), 'widget{Enter}');

    await waitFor(() =>
      expect(lastSearch()?.wanted).toEqual({
        branch: 'b'.repeat(40),
        author: 'Grace Hopper',
        subject: 'widget',
        file: null,
      }),
    );
  });

  it('returns to ordinary pagination when the filters are cleared', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Ganeshh');
    await waitFor(() => expect(lastSearch()).toBeDefined());

    await userEvent.click(screen.getByRole('button', { name: 'Clear filters' }));

    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Clear filters' })).not.toBeInTheDocument(),
    );
    expect(screen.queryByText(/of \d+ examined/)).not.toBeInTheDocument();
    expect(await screen.findByRole('list', { name: 'Commits' })).toBeInTheDocument();
    expect(screen.getByRole('searchbox', { name: 'Search' })).toHaveValue('');
  });

  it('offers the graph again once the filters are gone', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    expect(screen.getByRole('button', { name: 'Graph' })).toBeInTheDocument();

    await pick('Branch', 'main');
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Graph' })).not.toBeInTheDocument(),
    );

    await userEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Graph' })).toBeInTheDocument(),
    );
  });

  it('names a reference’s kind in a word, not only by where it sits', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await userEvent.click(screen.getByRole('button', { name: /^Branch/ }));

    expect(screen.getByRole('menuitem', { name: /v1\.0/ })).toHaveTextContent('tag');
  });

  it('says the author list is only as deep as it looked', async () => {
    searchable();
    backend([page()], {
      git_refs: REFS,
      git_changes: CHANGES,
      git_authors: {
        ...AUTHORS,
        scanned: 2000,
        stopped: { state: 'budget', scanned: 2000, limit: 2000 },
      } satisfies KnownAuthors,
    });
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await userEvent.click(screen.getByRole('button', { name: /^Author/ }));

    expect(
      screen.getByText(
        /Authors of the last 2000 commits\. Somebody further back may be missing/,
      ),
    ).toBeInTheDocument();
  });

  it('closes a menu with Escape and gives the focus back', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    const trigger = screen.getByRole('button', { name: /^Author/ });
    await userEvent.click(trigger);
    expect(screen.getByRole('menu', { name: 'Author' })).toBeInTheDocument();

    await userEvent.keyboard('{Escape}');

    expect(screen.queryByRole('menu', { name: 'Author' })).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it('moves through a menu with the arrow keys', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await userEvent.click(screen.getByRole('button', { name: /^Author/ }));
    const items = screen.getAllByRole('menuitem');

    items[0]!.focus();
    await userEvent.keyboard('{ArrowDown}');
    expect(items[1]).toHaveFocus();

    await userEvent.keyboard('{ArrowUp}');
    expect(items[0]).toHaveFocus();
  });

  it('says what is being filtered by, in words', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Grace Hopper');

    await waitFor(() =>
      expect(screen.getByText('Filtering by author Grace Hopper.')).toBeInTheDocument(),
    );
  });
});

describe('what a search found, and how far it looked', () => {
  it('counts the matches against the commits examined', async () => {
    searchable([found({ commits: commits(2), scanned: 340 })]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Ganeshh');

    expect(
      await screen.findByText('2 commits matching author Ganeshh, of 340 examined.'),
    ).toBeInTheDocument();
  });

  it('never says “no results” when it means “none yet”', async () => {
    // The whole point of the slice, in one assertion.
    searchable([
      found({
        commits: [],
        scanned: 2000,
        stopped: { state: 'budget', scanned: 2000, limit: 2000 },
        next: { from: 'e'.repeat(40), file: null },
      }),
    ]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Grace Hopper');

    expect(await screen.findByText('No match yet')).toBeInTheDocument();
    expect(
      screen.getByText(
        /Nothing matched in the 2000 commits examined\. There may be more further back/,
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText('No matching commits')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keep looking' })).toBeInTheDocument();
  });

  it('does say “no results” when it reached the end', async () => {
    searchable([found({ commits: [], scanned: 42, stopped: { state: 'no' }, next: null })]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Grace Hopper');

    expect(await screen.findByText('No matching commits')).toBeInTheDocument();
    expect(
      screen.getByText('Nothing in this history matches author Grace Hopper.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Keep looking' })).not.toBeInTheDocument();
  });

  it('says a partial page is partial even when it found something', async () => {
    searchable([
      found({
        scanned: 2000,
        stopped: { state: 'budget', scanned: 2000, limit: 2000 },
        next: { from: 'e'.repeat(40), file: null },
      }),
    ]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Author', 'Ganeshh');

    expect(
      await screen.findByText(
        /Stopped after examining 2000 commits, so this is what matched so far/,
      ),
    ).toBeInTheDocument();
  });

  it('continues from the cursor, carrying the file the search is now about', async () => {
    // Rename following across a page boundary: the second request asks about the
    // file the backend named, which is not the one the menu chose.
    const older = {
      scope: { kind: 'commit' as const, commit: 'f'.repeat(40) },
      at: 7,
      before: true,
    };
    searchable([
      found({ next: { from: 'e'.repeat(40), file: older } }),
      found({ commits: commits(1, 9), next: null }),
    ]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('File', 'src/app.ts');
    await screen.findByRole('button', { name: 'Keep looking' });
    await userEvent.click(screen.getByRole('button', { name: 'Keep looking' }));

    await waitFor(() =>
      expect(lastSearch()).toEqual({
        projectId: 1,
        wanted: { branch: null, author: null, subject: null, file: older },
        cursor: 'e'.repeat(40),
      }),
    );
  });

  it('says when a rename could not be followed', async () => {
    searchable([found({ stopped: { state: 'renameLost', path: 'src/app.ts' }, next: null })]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('File', 'src/app.ts');

    expect(await screen.findByText(/so the search ends at/)).toBeInTheDocument();
  });

  it('says a stale file selection is stale rather than failing', async () => {
    searchable([{ state: 'unknown' }]);
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('File', 'docs/notes.md');

    expect(await screen.findByText('That file is no longer in this list')).toBeInTheDocument();
  });

  it('offers nothing that would change the repository while filtering', async () => {
    searchable();
    show();
    await screen.findByRole('list', { name: 'Commits' });

    await pick('Branch', 'feature/graph');
    await screen.findByRole('list', { name: 'Commits' });

    for (const write of [/check ?out/i, /switch/i, /merge/i, /rebase/i, /reset/i, /^pull$/i]) {
      expect(screen.queryByRole('button', { name: write })).not.toBeInTheDocument();
    }
  });
});
