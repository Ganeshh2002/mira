import { useInfiniteQuery } from '@tanstack/react-query';
import { useRef } from 'react';

import type { CommitId } from '../bindings/CommitId';
import type { FileCommit } from '../bindings/FileCommit';
import type { FileSubject } from '../bindings/FileSubject';
import { Button } from './Button';
import { Icon } from './Icon';
import { commands, describeUnknown } from '../lib/ipc';
import { absoluteTime, relativeTime } from '../lib/time';

/**
 * The commits that touched one file.
 *
 * **The file is never named by a path.** This component receives a `subject` — a
 * change set Mira produced and a position in it — and hands it straight back. It
 * cannot construct one, so it cannot ask about a file Mira did not offer it
 * ([ADR-0017](../../docs/adr/0017-file-history.md)).
 *
 * **A page that stopped early says so.** File history is the one read in Mira that
 * is inherently proportional to the repository, so a request examines a bounded
 * number of commits and then reports how far it got. "Nothing found in the first
 * two thousand commits" and "this file has no history" are different sentences,
 * and confusing them would be the worst thing this surface could do.
 *
 * **Clicking a commit goes to that commit**, which is the existing detail surface
 * rather than a new one.
 */
export function FileHistory({
  projectId,
  subject,
  path,
  onOpenCommit,
}: {
  projectId: number;
  /** The file, as Mira named it. Echoed back, never built here. */
  subject: FileSubject;
  /** The path, for the labels. Display only; it is never sent. */
  path: string;
  /** Go to a commit's own detail, where this trace came from. */
  onOpenCommit: (commit: CommitId) => void;
}) {
  const list = useRef<HTMLUListElement>(null);

  const history = useInfiniteQuery({
    queryKey: ['git', 'fileHistory', projectId, subject],
    queryFn: ({ pageParam }) =>
      commands.gitFileHistory(projectId, pageParam.subject, pageParam.from),
    initialPageParam: { subject, from: null as CommitId | null },
    getNextPageParam: (last) =>
      last.state === 'ready' && last.next
        ? { subject: last.next.subject, from: last.next.from }
        : null,
  });

  const pages = history.data?.pages ?? [];
  const first = pages[0];
  const commits: FileCommit[] = pages.flatMap((page) =>
    page.state === 'ready' ? page.commits : [],
  );
  const scanned = pages.reduce(
    (total, page) => total + (page.state === 'ready' ? page.scanned : 0),
    0,
  );
  const last = pages[pages.length - 1];
  const stopped = last?.state === 'ready' ? last.stopped : null;

  function onKeyDown(event: React.KeyboardEvent<HTMLUListElement>) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;

    const rows = Array.from(
      list.current?.querySelectorAll<HTMLButtonElement>('button[data-file-commit]') ?? [],
    );
    const index = rows.indexOf(document.activeElement as HTMLButtonElement);
    if (index === -1) return;

    const moving = rows[event.key === 'ArrowDown' ? index + 1 : index - 1];
    if (!moving) return;

    event.preventDefault();
    moving.focus();
  }

  if (history.isPending) {
    return <Note>Looking through the history…</Note>;
  }
  if (history.isError) {
    return (
      <Note tone="danger" alert>
        {describeUnknown(history.error)}
      </Note>
    );
  }

  if (first?.state === 'notARepository') return <Note>Not a repository.</Note>;
  if (first?.state === 'unknown') {
    return <Note>That file is no longer in this list. Refresh to see what is there now.</Note>;
  }
  if (first?.state === 'unreadable') {
    return (
      <Note tone="warn" alert>
        {first.detail}
      </Note>
    );
  }

  return (
    <div className="flex flex-col gap-[var(--space-2)] border-t border-line bg-ground-0 px-[var(--space-3)] py-[var(--space-2)]">
      <p className="t-ui m-0 flex flex-wrap items-center gap-[var(--space-2)] text-ink-1">
        <Icon name="trace" />
        History of <span className="t-micro text-ink-0">{path}</span>
      </p>

      {commits.length === 0 ? (
        <p className="t-ui m-0 text-ink-2">
          {stopped?.state === 'budget'
            ? `Nothing in the last ${stopped.scanned} commits. There may be more further back.`
            : 'No commit has touched this file.'}
        </p>
      ) : (
        <ul
          ref={list}
          aria-label={`History of ${path}`}
          onKeyDown={onKeyDown}
          className="m-0 flex list-none flex-col overflow-hidden rounded-sm border border-line bg-ground-1 p-0"
        >
          {commits.map((found) => (
            <FileCommitRow
              key={`${found.commit.sha}-${found.path}`}
              found={found}
              onOpen={() => onOpenCommit(found.commit.sha)}
            />
          ))}
        </ul>
      )}

      {/*
        "Stopped early" and "nothing there" are different answers, and the
        difference matters most for exactly the files the bound exists for.
      */}
      {stopped?.state === 'budget' && commits.length > 0 ? (
        <p className="t-ui m-0 text-ink-2">
          Stopped after looking at {scanned} commits. There may be more further back.
        </p>
      ) : null}
      {stopped?.state === 'renameLost' ? (
        <p className="t-ui m-0 text-ink-2">
          This file was renamed in a commit that changed more paths than Mira reads at once, so
          the trail ends at <span className="t-micro">{stopped.path}</span>. Its history
          continues under an earlier name.
        </p>
      ) : null}
      {first?.state === 'ready' && first.shallow ? (
        <p className="t-ui m-0 text-ink-2">
          This is a shallow copy, so the oldest commit here is where the clone stops — not where
          the history does.
        </p>
      ) : null}

      {history.hasNextPage ? (
        <div>
          <Button
            onClick={() => void history.fetchNextPage()}
            disabled={history.isFetchingNextPage}
          >
            {history.isFetchingNextPage ? 'Looking further back…' : 'Look further back'}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

/**
 * One commit in a file's history.
 *
 * Subject, author, how long ago, and the abbreviated id — the same four facts
 * every commit row in Mira shows, so the surfaces read alike. A rename says so in
 * words as well as with an icon, because an arrow alone is a code
 * (`design-system.md` §5).
 */
function FileCommitRow({ found, onOpen }: { found: FileCommit; onOpen: () => void }) {
  const moved = found.renamedFrom;

  return (
    <li className="border-b border-line last:border-b-0">
      <button
        type="button"
        data-file-commit={found.commit.sha}
        onClick={onOpen}
        className="flex w-full cursor-default flex-col items-start gap-[var(--space-1)] px-[var(--space-3)] py-[var(--space-2)] text-left hover:bg-ground-2"
      >
        <span className="t-ui w-full truncate text-ink-0" title={found.commit.subject}>
          {found.commit.subject || '(no message)'}
        </span>
        <span className="t-micro flex flex-wrap items-center gap-[var(--space-3)] text-ink-2">
          <span className="flex items-center gap-[var(--space-1)]">
            <Icon name="person" />
            {found.commit.author}
          </span>
          <span
            className="flex items-center gap-[var(--space-1)]"
            title={absoluteTime(found.commit.committedAt)}
          >
            <Icon name="clock" />
            {relativeTime(found.commit.committedAt)}
          </span>
          <span>{found.commit.shortSha}</span>
          {moved ? (
            <span className="flex items-center gap-[var(--space-1)] text-ink-1">
              <Icon name="rename" />
              {found.kind === 'copied' ? 'Copied from' : 'Renamed from'} {moved}
            </span>
          ) : null}
        </span>
      </button>
    </li>
  );
}

/** A short, quiet line — never an alert unless it is one. */
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
