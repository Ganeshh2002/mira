import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';

import type { ChangeKind } from '../bindings/ChangeKind';
import type { CommitId } from '../bindings/CommitId';
import type { Comparison } from '../bindings/Comparison';
import type { DiffScope } from '../bindings/DiffScope';
import type { FileChange } from '../bindings/FileChange';
import { Icon } from './Icon';
import { Row } from './Row';
import { Section } from './Section';
import { FileHistory } from './FileHistory';
import { FilePatch } from './FilePatch';
import { commands, describeUnknown } from '../lib/ipc';

/**
 * What changed — a commit's files, or the working tree's.
 *
 * **A view, never an edit.** No staging, no checkout, no revert, no apply, and no
 * disabled control implying one later. Mira reads what Git holds and changes
 * nothing ([ADR-0016](../../docs/adr/0016-bounded-diffs.md)).
 *
 * **A file is chosen by its place in this list.** Selecting a row sends the
 * ordinal Mira gave it, never a path — so the interface can only ask for a file it
 * was already offered.
 *
 * **Every limit says so.** A change set that stops at two hundred files says how
 * many there were; a patch that stops at two thousand lines says so; a binary file
 * is named as binary and a huge one as too large. Nothing here is shortened
 * quietly, because a truncated diff that looked complete would be worse than none.
 */
export function Changes({
  projectId,
  scope,
  label,
  onOpenCommit,
}: {
  projectId: number;
  scope: DiffScope;
  /** What this change set is, for the section heading. */
  label: string;
  /**
   * Go to a commit's own detail. Given when this list sits somewhere a commit
   * can be opened from; without it, a file's history is shown and its rows are
   * not links to anywhere.
   */
  onOpenCommit?: ((commit: CommitId) => void) | undefined;
}) {
  const [opened, setOpened] = useState<number | null>(null);
  const [traced, setTraced] = useState<number | null>(null);

  const changes = useQuery({
    queryKey: ['git', 'changes', projectId, scope],
    queryFn: () => commands.gitChanges(projectId, scope),
  });

  if (changes.isPending) {
    return (
      <Section label={label}>
        <Row label="Reading…" />
      </Section>
    );
  }

  if (changes.isError) {
    return (
      <Section label={label}>
        <Row
          mark={<span className="text-signal-warn">◐</span>}
          label="Cannot be read"
          detail={describeUnknown(changes.error)}
        />
      </Section>
    );
  }

  const found = changes.data;

  if (found.state === 'notARepository') {
    return (
      <Section label={label}>
        <Row mark={<span className="text-ink-3">○</span>} label="Not a repository" />
      </Section>
    );
  }
  if (found.state === 'unknown') {
    return (
      <Section label={label}>
        <Row
          mark={<span className="text-ink-3">○</span>}
          label="Not in this repository"
          detail="It may have been rewritten, or this copy may not go back that far."
        />
      </Section>
    );
  }
  if (found.state === 'unreadable') {
    return (
      <Section label={label}>
        <Row
          mark={<span className="text-signal-warn">◐</span>}
          label="Cannot be read"
          detail={found.detail}
        />
      </Section>
    );
  }

  if (found.files.length === 0) {
    return (
      <Section label={label}>
        <Row
          mark={<span className="text-ink-3">○</span>}
          label={emptyLabel(found.against)}
          detail={comparedWith(found.against)}
        />
      </Section>
    );
  }

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <div className="flex flex-wrap items-baseline justify-between gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">{label}</h2>
        <span className="t-ui text-ink-2">{comparedWith(found.against)}</span>
      </div>

      <ul
        aria-label={label}
        className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
      >
        {found.files.map((change) => (
          <ChangedFileRow
            key={`${change.at}-${change.path}`}
            projectId={projectId}
            scope={scope}
            change={change}
            opened={opened === change.at}
            traced={traced === change.at}
            onOpen={() => setOpened(opened === change.at ? null : change.at)}
            onTrace={() => setTraced(traced === change.at ? null : change.at)}
            onOpenCommit={onOpenCommit}
          />
        ))}
      </ul>

      {found.truncated.state === 'yes' ? (
        <p className="t-ui m-0 text-ink-2">
          Showing {found.truncated.shown} of {found.truncated.total} changed files. Mira lists
          at most {found.truncated.limit} at a time.
        </p>
      ) : null}
    </section>
  );
}

/**
 * One changed path.
 *
 * `M src/app.ts +24 −8` — the letter and the word both, because `C` and `M` are
 * indistinguishable to somebody who has not memorised them, and colour alone is
 * never a status (`design-system.md` §5).
 */
function ChangedFileRow({
  projectId,
  scope,
  change,
  opened,
  traced,
  onOpen,
  onTrace,
  onOpenCommit,
}: {
  projectId: number;
  scope: DiffScope;
  change: FileChange;
  opened: boolean;
  traced: boolean;
  onOpen: () => void;
  onTrace: () => void;
  onOpenCommit?: ((commit: CommitId) => void) | undefined;
}) {
  return (
    <li className="flex flex-col border-b border-line last:border-b-0">
      <div
        className={`flex items-center ${opened || traced ? 'bg-ember-wash' : 'hover:bg-ground-2'}`}
      >
        <button
          type="button"
          onClick={onOpen}
          aria-expanded={opened}
          data-change={change.at}
          className="flex min-h-[var(--row-height)] min-w-0 flex-1 cursor-default items-center gap-[var(--space-3)] px-[var(--space-3)] py-[var(--space-2)] text-left"
        >
          <span
            aria-hidden="true"
            className={`t-micro w-[1.25em] shrink-0 text-center ${tone(change.kind)}`}
          >
            {letter(change.kind)}
          </span>

          <span className="flex min-w-0 flex-1 flex-col gap-[var(--space-1)]">
            <span className="t-ui min-w-0 truncate text-ink-0" title={change.path}>
              {change.path}
            </span>
            {change.fromPath ? (
              <span className="t-micro flex items-center gap-[var(--space-1)] text-ink-2">
                <Icon name="rename" />
                from {change.fromPath}
              </span>
            ) : null}
          </span>

          <span className="t-micro flex shrink-0 items-center gap-[var(--space-2)]">
            {/* The word, so the letter never has to be decoded. */}
            <span className="text-ink-2">{word(change.kind)}</span>
            {change.binary ? <span className="text-ink-2">binary</span> : null}
            {change.additions !== null ? (
              <span className="text-signal-ok">+{change.additions}</span>
            ) : null}
            {change.deletions !== null ? (
              <span className="text-signal-danger">−{change.deletions}</span>
            ) : null}
          </span>
        </button>

        {/*
        A second disclosure rather than a second surface. The file is named to
        the backend by its **ordinal in this list**, which the row already
        knows — no path is built here, because none can be.
      */}
        <button
          type="button"
          onClick={onTrace}
          aria-expanded={traced}
          aria-label={`File history of ${change.path}`}
          title={`File history of ${change.path}`}
          className="mr-[var(--space-2)] flex shrink-0 cursor-default items-center gap-[var(--space-1)] rounded-sm px-[var(--space-2)] py-[var(--space-1)] text-ink-2 hover:bg-ground-3 hover:text-ink-0"
        >
          <Icon name="trace" />
          <span className="t-micro hidden sm:inline">History</span>
        </button>
      </div>

      {traced ? (
        <FileHistory
          projectId={projectId}
          subject={{ scope, at: change.at, before: false }}
          path={change.path}
          onOpenCommit={onOpenCommit ?? (() => {})}
        />
      ) : null}

      {opened ? (
        <FilePatch projectId={projectId} scope={scope} at={change.at} path={change.path} />
      ) : null}
    </li>
  );
}

function letter(kind: ChangeKind): string {
  switch (kind) {
    case 'added':
      return 'A';
    case 'deleted':
      return 'D';
    case 'modified':
      return 'M';
    case 'renamed':
      return 'R';
    case 'copied':
      return 'C';
    case 'typeChanged':
      return 'T';
  }
}

function word(kind: ChangeKind): string {
  switch (kind) {
    case 'added':
      return 'Added';
    case 'deleted':
      return 'Deleted';
    case 'modified':
      return 'Modified';
    case 'renamed':
      return 'Renamed';
    case 'copied':
      return 'Copied';
    case 'typeChanged':
      return 'Type changed';
  }
}

/** Colour is the third channel here, after the letter and the word. */
function tone(kind: ChangeKind): string {
  switch (kind) {
    case 'added':
      return 'text-signal-ok';
    case 'deleted':
      return 'text-signal-danger';
    default:
      return 'text-ink-1';
  }
}

/**
 * What this change set was compared against.
 *
 * A merge is compared against its **first** parent, and this is where that is
 * said: against the other parent the same commit changed different things, and a
 * diff that did not say which side it picked would be quietly choosing one.
 */
function comparedWith(against: Comparison): string {
  switch (against.kind) {
    case 'emptyTree':
      return 'The first commit — everything in it is new';
    case 'parent':
      return 'Compared with its parent';
    case 'firstParent':
      return `A merge, compared with the first of its ${against.parents} parents`;
    case 'head':
      return 'Compared with HEAD';
  }
}

function emptyLabel(against: Comparison): string {
  return against.kind === 'head' ? 'Nothing has changed' : 'This commit changed nothing';
}
