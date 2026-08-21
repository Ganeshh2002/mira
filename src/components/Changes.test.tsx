import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ChangedFiles } from '../bindings/ChangedFiles';
import type { FileChange } from '../bindings/FileChange';
import type { FileDiff } from '../bindings/FileDiff';
import { Changes } from './Changes';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function change(overrides: Partial<FileChange> = {}): FileChange {
  return {
    at: 0,
    kind: 'modified',
    path: 'src/app.ts',
    fromPath: null,
    binary: false,
    additions: 24,
    deletions: 8,
    ...overrides,
  };
}

function listed(files: FileChange[], overrides: Partial<ChangedFiles> = {}): ChangedFiles {
  return {
    state: 'ready',
    files,
    against: { kind: 'parent' },
    truncated: { state: 'no' },
    ...overrides,
  } as ChangedFiles;
}

/** Serve a change list, and whatever patch is asked for. */
function backend(changes: ChangedFiles, patches: Record<number, FileDiff> = {}) {
  invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
    if (command === 'git_changes') return Promise.resolve(changes);
    if (command === 'git_file_diff') {
      const at = (args as { at?: number } | undefined)?.at ?? 0;
      return Promise.resolve(patches[at] ?? { state: 'unknown' });
    }
    return Promise.resolve(undefined);
  });
}

function show(scope: 'commit' | 'workingTree' = 'commit') {
  return renderApp(
    <Changes
      projectId={1}
      scope={
        scope === 'commit' ? { kind: 'commit', commit: 'ad50fc7' } : { kind: 'workingTree' }
      }
      label="Changed files"
    />,
  );
}

/**
 * The row buttons, without the per-row File history control beside them.
 *
 * A row carries two disclosures now — the patch and the file's history — so
 * "the button" is ambiguous. The patch one is the row itself, and it is the one
 * that knows its ordinal.
 */
function rows(list: HTMLElement) {
  return within(list)
    .getAllByRole('button')
    .filter((button) => button.hasAttribute('data-change'));
}

beforeEach(() => {
  invoke.mockReset();
});

describe('the changed-file list', () => {
  it('shows the letter, the word, and the counts', async () => {
    // `M src/app.ts +24 −8`. The letter alone is a code somebody has to have
    // memorised, so the word is there too (`design-system.md` §5).
    backend(listed([change()]));
    show();

    const row = within(await screen.findByRole('list', { name: 'Changed files' })).getByRole(
      'listitem',
    );

    expect(row).toHaveTextContent('M');
    expect(row).toHaveTextContent('Modified');
    expect(row).toHaveTextContent('src/app.ts');
    expect(row).toHaveTextContent('+24');
    expect(row).toHaveTextContent('−8');
  });

  it('names every kind of change in words', async () => {
    backend(
      listed([
        change({ at: 0, kind: 'added', path: 'a.ts', deletions: 0 }),
        change({ at: 1, kind: 'deleted', path: 'b.ts', additions: 0 }),
        change({ at: 2, kind: 'renamed', path: 'new.ts', fromPath: 'old.ts' }),
        change({ at: 3, kind: 'copied', path: 'copy.ts', fromPath: 'src.ts' }),
        change({ at: 4, kind: 'typeChanged', path: 'link' }),
      ]),
    );
    show();

    const rows = within(
      await screen.findByRole('list', { name: 'Changed files' }),
    ).getAllByRole('listitem');

    expect(rows[0]).toHaveTextContent('Added');
    expect(rows[1]).toHaveTextContent('Deleted');
    expect(rows[2]).toHaveTextContent('Renamed');
    expect(rows[3]).toHaveTextContent('Copied');
    expect(rows[4]).toHaveTextContent('Type changed');
  });

  it('says where a renamed file came from', async () => {
    backend(listed([change({ kind: 'renamed', path: 'src/new.ts', fromPath: 'src/old.ts' })]));
    show();

    expect(await screen.findByText(/from src\/old\.ts/)).toBeInTheDocument();
  });

  it('marks a binary file and shows no counts for it', async () => {
    backend(
      listed([change({ binary: true, path: 'logo.png', additions: null, deletions: null })]),
    );
    show();

    const row = within(await screen.findByRole('list', { name: 'Changed files' })).getByRole(
      'listitem',
    );

    expect(row).toHaveTextContent('binary');
    expect(row).not.toHaveTextContent('+');
  });

  it('says which side a merge was compared against', async () => {
    // Against the other parent the same commit changed different things, so a
    // diff that did not say which side it picked would be choosing one quietly.
    backend(listed([change()], { against: { kind: 'firstParent', parents: 2 } }));
    show();

    expect(
      await screen.findByText('A merge, compared with the first of its 2 parents'),
    ).toBeInTheDocument();
  });

  it('says an empty commit changed nothing rather than showing a blank list', async () => {
    backend(listed([]));
    show();

    expect(await screen.findByText('This commit changed nothing')).toBeInTheDocument();
  });

  it('says a clean working tree is clean', async () => {
    backend(listed([], { against: { kind: 'head' } }));
    show('workingTree');

    expect(await screen.findByText('Nothing has changed')).toBeInTheDocument();
  });
});

describe('the limits', () => {
  it('says how many files there were when the list stops short', async () => {
    backend(
      listed([change()], {
        truncated: { state: 'yes', shown: 200, total: 4312, limit: 200 },
      }),
    );
    show();

    expect(await screen.findByText(/Showing 200 of 4312 changed files/)).toBeInTheDocument();
    expect(screen.getByText(/at most 200 at a time/)).toBeInTheDocument();
  });

  it('says nothing about limits when none of them bit', async () => {
    backend(listed([change()]));
    show();

    await screen.findByRole('list', { name: 'Changed files' });
    expect(screen.queryByText(/Showing/)).not.toBeInTheDocument();
  });
});

describe('opening a file', () => {
  const patch: FileDiff = {
    state: 'ready',
    change: change(),
    hunks: [
      {
        header: '@@ -1,3 +1,3 @@',
        lines: [
          { kind: 'context', oldLine: 1, newLine: 1, text: 'one', cut: false },
          { kind: 'deletion', oldLine: 2, newLine: null, text: 'two', cut: false },
          { kind: 'addition', oldLine: null, newLine: 2, text: 'TWO', cut: false },
        ],
      },
    ],
    truncated: { state: 'no' },
  };

  it('asks for the file by its place in the list, never by its path', async () => {
    // The whole file-selection contract, from the interface's side.
    backend(listed([change({ at: 0 }), change({ at: 1, path: 'src/other.ts' })]), {
      1: patch,
    });
    show();

    const found = rows(await screen.findByRole('list', { name: 'Changed files' }));
    await userEvent.click(found[1]!);

    await waitFor(() => {
      const asked = invoke.mock.calls.find(([command]) => command === 'git_file_diff');
      expect(asked?.[1]).toEqual({
        projectId: 1,
        scope: { kind: 'commit', commit: 'ad50fc7' },
        at: 1,
      });
    });

    const sent = invoke.mock.calls
      .filter(([command]) => command === 'git_file_diff')
      .flatMap(([, args]) => Object.keys((args ?? {}) as Record<string, unknown>));
    expect(sent).not.toContain('path');
    expect(sent).not.toContain('file');
  });

  it('shows the patch with line numbers on both sides', async () => {
    backend(listed([change()]), { 0: patch });
    show();

    await userEvent.click(rows(await screen.findByRole('list', { name: 'Changed files' }))[0]!);

    const table = await screen.findByRole('table', { name: /Changes to src\/app\.ts/ });
    expect(within(table).getByText('@@ -1,3 +1,3 @@')).toBeInTheDocument();
    expect(within(table).getByText('two')).toBeInTheDocument();
    expect(within(table).getByText('TWO')).toBeInTheDocument();
  });

  it('reads a patch out as a table rather than a wall of characters', async () => {
    // A screen reader should hear "added line 2: TWO", not a run of punctuation.
    backend(listed([change()]), { 0: patch });
    show();

    await userEvent.click(rows(await screen.findByRole('list', { name: 'Changed files' }))[0]!);

    expect(await screen.findByText('Added line 2:')).toBeInTheDocument();
    expect(screen.getByText('Removed line 2:')).toBeInTheDocument();
  });

  it('closes again when the row is clicked a second time', async () => {
    backend(listed([change()]), { 0: patch });
    show();

    const row = rows(await screen.findByRole('list', { name: 'Changed files' }))[0]!;
    await userEvent.click(row);
    await screen.findByRole('table');

    await userEvent.click(row);
    await waitFor(() => expect(screen.queryByRole('table')).not.toBeInTheDocument());
  });
});

describe('a patch that cannot be shown whole', () => {
  async function open(diff: FileDiff) {
    backend(listed([change()]), { 0: diff });
    show();
    await userEvent.click(rows(await screen.findByRole('list', { name: 'Changed files' }))[0]!);
  }

  it('identifies a binary file rather than decoding it', async () => {
    await open({
      state: 'binary',
      change: change({ binary: true, path: 'logo.png' }),
      oldBytes: 1024,
      newBytes: 2048,
    });

    expect(await screen.findByText(/Binary file/)).toBeInTheDocument();
    expect(screen.getByText(/1 KB → 2 KB/)).toBeInTheDocument();
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  });

  it('says a file was too large to read at all', async () => {
    await open({
      state: 'tooLarge',
      change: change({ path: 'huge.json' }),
      bytes: 8 * 1024 * 1024,
      limit: 2 * 1024 * 1024,
    });

    expect(await screen.findByText(/8\.0 MB is larger than the 2\.0 MB/)).toBeInTheDocument();
    expect(screen.getByText(/this file was not read/)).toBeInTheDocument();
  });

  it('says how many lines were shown when the line limit bit', async () => {
    await open({
      state: 'ready',
      change: change(),
      hunks: [
        {
          header: '@@ -1,1 +1,1 @@',
          lines: [{ kind: 'addition', oldLine: null, newLine: 1, text: 'x', cut: false }],
        },
      ],
      truncated: { state: 'lines', shown: 2000, limit: 2000 },
    });

    expect(await screen.findByText(/Showing the first 2000 lines/)).toBeInTheDocument();
  });

  it('says how much was shown when the byte limit bit', async () => {
    await open({
      state: 'ready',
      change: change(),
      hunks: [
        {
          header: '@@ -1,1 +1,1 @@',
          lines: [{ kind: 'addition', oldLine: null, newLine: 1, text: 'x', cut: false }],
        },
      ],
      truncated: { state: 'bytes', shown: 262144, limit: 262144 },
    });

    expect(await screen.findByText(/Showing the first 256 KB/)).toBeInTheDocument();
  });

  it('marks a line that was cut', async () => {
    await open({
      state: 'ready',
      change: change(),
      hunks: [
        {
          header: '@@ -1,1 +1,1 @@',
          lines: [{ kind: 'addition', oldLine: null, newLine: 1, text: 'xxxx', cut: true }],
        },
      ],
      truncated: { state: 'no' },
    });

    expect(await screen.findByText(/line cut at 2000 characters/)).toBeInTheDocument();
  });

  it('says a stale selection is stale rather than failing', async () => {
    await open({ state: 'unknown' });

    expect(await screen.findByText(/no longer in this list/)).toBeInTheDocument();
  });

  it('says a rename with no edits has nothing inside it', async () => {
    await open({
      state: 'ready',
      change: change({ kind: 'renamed', fromPath: 'src/old.ts' }),
      hunks: [],
      truncated: { state: 'no' },
    });

    expect(
      await screen.findByText(/Moved, with no change inside the file/),
    ).toBeInTheDocument();
  });
});

describe('what a diff never offers', () => {
  it('has no control that would change the repository', async () => {
    backend(listed([change()]), {
      0: {
        state: 'ready',
        change: change(),
        hunks: [
          {
            header: '@@ -1,1 +1,1 @@',
            lines: [{ kind: 'addition', oldLine: null, newLine: 1, text: 'x', cut: false }],
          },
        ],
        truncated: { state: 'no' },
      },
    });
    show();

    await userEvent.click(rows(await screen.findByRole('list', { name: 'Changed files' }))[0]!);
    await screen.findByRole('table');

    // Absent, not disabled: a greyed-out Stage would promise a later release
    // (`information-architecture.md` §5).
    for (const write of [
      /stage/i,
      /unstage/i,
      /discard/i,
      /revert/i,
      /apply/i,
      /check ?out/i,
      /reset/i,
      /commit/i,
      /edit/i,
    ]) {
      expect(screen.queryByRole('button', { name: write })).not.toBeInTheDocument();
    }
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});

describe('reaching a file’s history', () => {
  it('offers it on every row, named for the file', async () => {
    backend(listed([change({ path: 'src/app.ts' })]));
    show();

    expect(
      await screen.findByRole('button', { name: 'File history of src/app.ts' }),
    ).toBeInTheDocument();
  });

  it('names the file by its ordinal in this list, never by its path', async () => {
    // The same contract as the patch, for the same reason: the row already knows
    // where it sits, so nothing has to describe where the file lives.
    invoke.mockImplementation((command: string) => {
      if (command === 'git_changes') return Promise.resolve(listed([change({ at: 4 })]));
      if (command === 'git_file_history') {
        return Promise.resolve({
          state: 'ready',
          path: 'src/app.ts',
          commits: [],
          next: null,
          scanned: 3,
          stopped: { state: 'no' },
          shallow: false,
        });
      }
      return Promise.resolve(undefined);
    });
    show();

    await userEvent.click(
      await screen.findByRole('button', { name: 'File history of src/app.ts' }),
    );

    await waitFor(() => {
      const asked = invoke.mock.calls.find(([command]) => command === 'git_file_history');
      expect(asked?.[1]).toEqual({
        projectId: 1,
        subject: { scope: { kind: 'commit', commit: 'ad50fc7' }, at: 4, before: false },
        cursor: null,
      });
    });

    const sent = JSON.stringify(
      invoke.mock.calls.find(([command]) => command === 'git_file_history')?.[1],
    );
    expect(sent).not.toContain('src/app.ts');
  });

  it('does not read a file’s history until somebody asks for it', async () => {
    backend(listed([change()]));
    show();

    await screen.findByRole('list', { name: 'Changed files' });
    expect(
      invoke.mock.calls.filter(([command]) => command === 'git_file_history'),
    ).toHaveLength(0);
  });

  it('closes again when the control is pressed a second time', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'git_changes') return Promise.resolve(listed([change()]));
      if (command === 'git_file_history') {
        return Promise.resolve({
          state: 'ready',
          path: 'src/app.ts',
          commits: [],
          next: null,
          scanned: 3,
          stopped: { state: 'no' },
          shallow: false,
        });
      }
      return Promise.resolve(undefined);
    });
    show();

    const control = await screen.findByRole('button', {
      name: 'File history of src/app.ts',
    });
    await userEvent.click(control);
    expect(await screen.findByText('No commit has touched this file.')).toBeInTheDocument();

    await userEvent.click(control);
    await waitFor(() =>
      expect(screen.queryByText('No commit has touched this file.')).not.toBeInTheDocument(),
    );
  });
});
