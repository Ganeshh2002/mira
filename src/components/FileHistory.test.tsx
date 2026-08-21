import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { FileCommit } from '../bindings/FileCommit';
import type { FileHistory as FileHistoryPage } from '../bindings/FileHistory';
import type { FileSubject } from '../bindings/FileSubject';
import { FileHistory } from './FileHistory';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const NOW = 1_800_000_000;

const subject: FileSubject = {
  scope: { kind: 'commit', commit: 'ad50fc7' },
  at: 3,
  before: false,
};

function touched(overrides: Partial<FileCommit> = {}): FileCommit {
  return {
    commit: {
      sha: 'a'.repeat(40),
      shortSha: 'aaaaaaa',
      subject: 'edit the file',
      author: 'Ganeshh',
      committedAt: NOW - 3600,
    },
    path: 'src/app.ts',
    kind: 'modified',
    renamedFrom: null,
    ...overrides,
  };
}

function page(overrides: Partial<Extract<FileHistoryPage, { state: 'ready' }>> = {}) {
  return {
    state: 'ready' as const,
    path: 'src/app.ts',
    commits: [touched()],
    next: null,
    scanned: 12,
    stopped: { state: 'no' as const },
    shallow: false,
    ...overrides,
  };
}

/** Serve one or more pages, in order. */
function backend(pages: FileHistoryPage[]) {
  let served = 0;
  invoke.mockImplementation((command: string) => {
    if (command === 'git_file_history') {
      const next = pages[Math.min(served, pages.length - 1)]!;
      served += 1;
      return Promise.resolve(next);
    }
    return Promise.resolve(undefined);
  });
}

function show(onOpenCommit = vi.fn()) {
  return renderApp(
    <FileHistory
      projectId={1}
      subject={subject}
      path="src/app.ts"
      onOpenCommit={onOpenCommit}
    />,
  );
}

beforeEach(() => {
  invoke.mockReset();
});

describe('a file’s history', () => {
  it('names the file by the subject it was given, never by a path', async () => {
    // The contract the whole slice rests on, from the interface's side.
    backend([page()]);
    show();

    await screen.findByRole('list', { name: 'History of src/app.ts' });

    const asked = invoke.mock.calls.find(([command]) => command === 'git_file_history');
    expect(asked?.[1]).toEqual({ projectId: 1, subject, cursor: null });

    const sent = Object.keys((asked?.[1] ?? {}) as Record<string, unknown>);
    expect(sent).not.toContain('path');
    expect(sent).not.toContain('pathspec');
    expect(JSON.stringify(asked?.[1])).not.toContain('src/app.ts');
  });

  it('shows the subject, author, relative time and short SHA', async () => {
    backend([page()]);
    show();

    const row = within(
      await screen.findByRole('list', { name: 'History of src/app.ts' }),
    ).getByRole('listitem');

    expect(row).toHaveTextContent('edit the file');
    expect(row).toHaveTextContent('Ganeshh');
    expect(row).toHaveTextContent(/ago|just now/);
    expect(row).toHaveTextContent('aaaaaaa');
  });

  it('says a rename in words, not only with an arrow', async () => {
    backend([
      page({
        commits: [
          touched({
            kind: 'renamed',
            renamedFrom: 'src/old.ts',
            commit: { ...touched().commit, subject: 'move it' },
          }),
        ],
      }),
    ]);
    show();

    expect(await screen.findByText(/Renamed from src\/old\.ts/)).toBeInTheDocument();
  });

  it('says a copy is a copy rather than a rename', async () => {
    backend([
      page({
        commits: [touched({ kind: 'copied', renamedFrom: 'src/source.ts' })],
      }),
    ]);
    show();

    expect(await screen.findByText(/Copied from src\/source\.ts/)).toBeInTheDocument();
  });

  it('says a file has no history rather than showing an empty list', async () => {
    backend([page({ commits: [] })]);
    show();

    expect(await screen.findByText('No commit has touched this file.')).toBeInTheDocument();
  });
});

describe('the scan budget', () => {
  it('distinguishes "nothing found yet" from "nothing there"', async () => {
    // The sentence that matters most for this feature: a trace that ran out of
    // budget looks exactly like a file with no history unless it says so.
    backend([
      page({
        commits: [],
        scanned: 2000,
        stopped: { state: 'budget', scanned: 2000, limit: 2000 },
        next: { subject, from: 'b'.repeat(40) },
      }),
    ]);
    show();

    expect(
      await screen.findByText(
        /Nothing in the last 2000 commits\. There may be more further back/,
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText('No commit has touched this file.')).not.toBeInTheDocument();
  });

  it('says how far it looked when it found some but stopped early', async () => {
    backend([
      page({
        scanned: 2000,
        stopped: { state: 'budget', scanned: 2000, limit: 2000 },
        next: { subject, from: 'b'.repeat(40) },
      }),
    ]);
    show();

    expect(
      await screen.findByText(/Stopped after looking at 2000 commits/),
    ).toBeInTheDocument();
  });

  it('says nothing about a budget when it reached the end', async () => {
    backend([page()]);
    show();

    await screen.findByRole('list', { name: 'History of src/app.ts' });
    expect(screen.queryByText(/Stopped after looking/)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Look further back' })).not.toBeInTheDocument();
  });

  it('continues from the cursor it was given, subject and all', async () => {
    // Rename following across a page boundary lives entirely in this echo: the
    // next page's subject is whatever the backend said, never something built
    // here.
    const older: FileSubject = {
      scope: { kind: 'commit', commit: 'b'.repeat(40) },
      at: 7,
      before: true,
    };
    backend([
      page({ next: { subject: older, from: 'c'.repeat(40) } }),
      page({
        commits: [touched({ path: 'src/old.ts' })],
        next: null,
      }),
    ]);
    show();

    await userEvent.click(await screen.findByRole('button', { name: 'Look further back' }));

    await waitFor(() => {
      const second = invoke.mock.calls.filter(([command]) => command === 'git_file_history')[1];
      expect(second?.[1]).toEqual({
        projectId: 1,
        subject: older,
        cursor: 'c'.repeat(40),
      });
    });
  });

  it('says when a rename could not be followed', async () => {
    backend([
      page({
        stopped: { state: 'renameLost', path: 'src/app.ts' },
        next: null,
      }),
    ]);
    show();

    expect(await screen.findByText(/the trail ends at/)).toBeInTheDocument();
    expect(screen.getByText(/continues under an earlier name/)).toBeInTheDocument();
  });

  it('says a shallow copy is shallow', async () => {
    backend([page({ shallow: true })]);
    show();

    expect(await screen.findByText(/shallow copy/)).toBeInTheDocument();
  });
});

describe('navigating', () => {
  it('opens a commit’s own detail when its row is clicked', async () => {
    const onOpenCommit = vi.fn();
    backend([page()]);
    show(onOpenCommit);

    await userEvent.click(
      within(await screen.findByRole('list', { name: 'History of src/app.ts' })).getByRole(
        'button',
      ),
    );

    expect(onOpenCommit).toHaveBeenCalledWith('a'.repeat(40));
  });

  it('moves between commits with the arrow keys', async () => {
    backend([
      page({
        commits: [
          touched({ commit: { ...touched().commit, sha: 'a'.repeat(40) } }),
          touched({
            commit: { ...touched().commit, sha: 'b'.repeat(40), subject: 'older' },
          }),
        ],
      }),
    ]);
    show();

    const rows = within(
      await screen.findByRole('list', { name: 'History of src/app.ts' }),
    ).getAllByRole('button');

    rows[0]!.focus();
    await userEvent.keyboard('{ArrowDown}');
    expect(rows[1]).toHaveFocus();

    await userEvent.keyboard('{ArrowDown}');
    expect(rows[1]).toHaveFocus();

    await userEvent.keyboard('{ArrowUp}');
    expect(rows[0]).toHaveFocus();
  });

  it('says a stale subject is stale rather than failing', async () => {
    backend([{ state: 'unknown' }]);
    show();

    expect(await screen.findByText(/no longer in this list/)).toBeInTheDocument();
  });

  it('shows the reason a repository could not be read', async () => {
    backend([{ state: 'unreadable', detail: 'HEAD does not name a reference.' }]);
    show();

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'HEAD does not name a reference.',
    );
  });
});

describe('what a file history never offers', () => {
  it('has no control that would change the repository', async () => {
    backend([page()]);
    show();

    await screen.findByRole('list', { name: 'History of src/app.ts' });

    for (const write of [
      /check ?out/i,
      /revert/i,
      /restore/i,
      /reset/i,
      /blame/i,
      /cherry.?pick/i,
      /^commit$/i,
    ]) {
      expect(screen.queryByRole('button', { name: write })).not.toBeInTheDocument();
    }
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});
