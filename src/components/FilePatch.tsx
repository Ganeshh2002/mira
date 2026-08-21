import { useQuery } from '@tanstack/react-query';

import type { DiffLine } from '../bindings/DiffLine';
import type { DiffScope } from '../bindings/DiffScope';
import type { Hunk } from '../bindings/Hunk';
import { Icon } from './Icon';
import { commands, describeUnknown } from '../lib/ipc';

/**
 * One file's patch.
 *
 * **Read-only, and structurally so.** There is nothing to type in and nothing to
 * press: the lines are text, the numbers are text, and no control here stages,
 * reverts, applies or edits anything.
 *
 * **Accessible as a table, because it is one.** Old line, new line, and the line
 * itself — three columns with headers, so a screen reader reads "line 42, added,
 * `const x = 1`" rather than a wall of characters. Additions and deletions are
 * marked with `+` and `−` as well as colour, so the patch survives a monochrome
 * display and every form of colour blindness (`design-system.md` §5).
 *
 * **Long lines scroll rather than wrap.** A wrapped diff loses the alignment that
 * makes it readable, so the table scrolls horizontally inside its own box and the
 * page does not.
 */
export function FilePatch({
  projectId,
  scope,
  at,
  path,
}: {
  projectId: number;
  scope: DiffScope;
  /** The file's position in the change list — never a path. */
  at: number;
  /** The path, for the labels. Display only; it is never sent back. */
  path: string;
}) {
  const diff = useQuery({
    queryKey: ['git', 'fileDiff', projectId, scope, at],
    queryFn: () => commands.gitFileDiff(projectId, scope, at),
  });

  if (diff.isPending) return <Note>Reading the patch…</Note>;
  if (diff.isError) {
    return (
      <Note tone="danger" alert>
        {describeUnknown(diff.error)}
      </Note>
    );
  }

  const found = diff.data;

  switch (found.state) {
    case 'notARepository':
      return <Note>Not a repository.</Note>;

    case 'unknown':
      return (
        <Note>That change is no longer in this list. Refresh to see what is there now.</Note>
      );

    case 'unreadable':
      return (
        <Note tone="warn" alert>
          {found.detail}
        </Note>
      );

    case 'binary':
      return (
        <Note>
          <span className="flex flex-wrap items-center gap-[var(--space-2)]">
            <Icon name="binary" />
            Binary file — Mira shows its size rather than decoding it.
            <span className="t-micro text-ink-2">{sizes(found.oldBytes, found.newBytes)}</span>
          </span>
        </Note>
      );

    case 'tooLarge':
      return (
        <Note>
          <span className="flex flex-wrap items-center gap-[var(--space-2)]">
            <Icon name="binary" />
            {kilobytes(found.bytes)} is larger than the {kilobytes(found.limit)} Mira will
            compare, so this file was not read.
          </span>
        </Note>
      );

    case 'ready':
      break;
  }

  if (found.hunks.length === 0) {
    return (
      <Note>
        {found.change.fromPath
          ? 'Moved, with no change inside the file.'
          : 'No textual change — the file’s mode or type changed.'}
      </Note>
    );
  }

  return (
    <div className="flex flex-col gap-[var(--space-2)] border-t border-line bg-ground-0 px-[var(--space-3)] py-[var(--space-2)]">
      {/*
        `overflow-x-auto` on the box, never on the page: a long line scrolls
        inside its own boundary and the surface around it stays put.
      */}
      <div className="overflow-x-auto rounded-sm border border-line">
        <table className="w-full border-collapse text-left">
          <caption className="sr-only">Changes to {path}</caption>
          <thead className="sr-only">
            <tr>
              <th scope="col">Line before</th>
              <th scope="col">Line after</th>
              <th scope="col">Change</th>
            </tr>
          </thead>
          {found.hunks.map((hunk, index) => (
            <HunkBody key={`${hunk.header}-${index}`} hunk={hunk} />
          ))}
        </table>
      </div>

      {found.truncated.state === 'lines' ? (
        <p className="t-ui m-0 text-ink-2">
          Showing the first {found.truncated.shown} lines. Mira renders at most{' '}
          {found.truncated.limit} of one file’s patch.
        </p>
      ) : null}
      {found.truncated.state === 'bytes' ? (
        <p className="t-ui m-0 text-ink-2">
          Showing the first {kilobytes(found.truncated.shown)}. Mira returns at most{' '}
          {kilobytes(found.truncated.limit)} of one file’s patch.
        </p>
      ) : null}
    </div>
  );
}

/** One hunk: a boundary a reader recognises, then its lines. */
function HunkBody({ hunk }: { hunk: Hunk }) {
  return (
    <tbody className="border-b border-line last:border-b-0">
      <tr>
        <th
          scope="colgroup"
          colSpan={3}
          className="t-micro bg-ground-2 px-[var(--space-2)] py-[var(--space-1)] text-left font-normal text-ink-2"
        >
          {hunk.header}
        </th>
      </tr>
      {hunk.lines.map((line, index) => (
        <PatchLine key={index} line={line} />
      ))}
    </tbody>
  );
}

function PatchLine({ line }: { line: DiffLine }) {
  const marks = {
    addition: { sign: '+', row: 'bg-signal-ok/10', text: 'text-ink-0' },
    deletion: { sign: '−', row: 'bg-signal-danger/10', text: 'text-ink-0' },
    context: { sign: ' ', row: '', text: 'text-ink-1' },
    note: { sign: ' ', row: '', text: 'text-ink-2' },
  }[line.kind];

  return (
    <tr className={marks.row}>
      <td className="t-micro w-[1px] select-none whitespace-nowrap px-[var(--space-2)] text-right align-top text-ink-3">
        {line.oldLine ?? ''}
      </td>
      <td className="t-micro w-[1px] select-none whitespace-nowrap px-[var(--space-2)] text-right align-top text-ink-3">
        {line.newLine ?? ''}
      </td>
      <td className={`t-micro whitespace-pre px-[var(--space-2)] align-top ${marks.text}`}>
        {/*
          The sign is part of the text, not a colour: the patch reads correctly
          in a screen reader and on a monochrome display.
        */}
        <span aria-hidden="true" className="select-none text-ink-3">
          {marks.sign}
        </span>
        <span className="sr-only">{describeLine(line)}</span>
        {line.text}
        {line.cut ? <span className="text-ink-3"> … line cut at 2000 characters</span> : null}
      </td>
    </tr>
  );
}

function describeLine(line: DiffLine): string {
  switch (line.kind) {
    case 'addition':
      return `Added line ${line.newLine ?? ''}: `;
    case 'deletion':
      return `Removed line ${line.oldLine ?? ''}: `;
    case 'context':
      return `Line ${line.newLine ?? line.oldLine ?? ''}: `;
    case 'note':
      return 'Note: ';
  }
}

/** A short, quiet line under a change — never an alert unless it is one. */
function Note({
  children,
  tone = 'quiet',
  alert = false,
}: {
  children: React.ReactNode;
  tone?: 'quiet' | 'warn' | 'danger';
  alert?: boolean;
}) {
  const colour = {
    quiet: 'text-ink-2',
    warn: 'text-signal-warn',
    danger: 'text-signal-danger',
  }[tone];

  return (
    <p
      role={alert ? 'alert' : undefined}
      className={`t-ui m-0 border-t border-line bg-ground-0 px-[var(--space-3)] py-[var(--space-2)] ${colour}`}
    >
      {children}
    </p>
  );
}

function kilobytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function sizes(before: number | null, after: number | null): string {
  if (before !== null && after !== null) {
    return `${kilobytes(before)} → ${kilobytes(after)}`;
  }
  if (after !== null) return `${kilobytes(after)} added`;
  if (before !== null) return `${kilobytes(before)} removed`;
  return '';
}
