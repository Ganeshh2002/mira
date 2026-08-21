import { useInfiniteQuery, useMutation, useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import type { Commit } from '../bindings/Commit';
import type { CommitId } from '../bindings/CommitId';
import type { Head } from '../bindings/Head';
import type { Project } from '../bindings/Project';
import type { RepositoryLayout } from '../bindings/RepositoryLayout';
import { Button } from '../components/Button';
import { Icon } from '../components/Icon';
import { Row } from '../components/Row';
import { Section } from '../components/Section';
import { commands, describeUnknown } from '../lib/ipc';
import { absoluteTime, relativeTime } from '../lib/time';

/**
 * What happened lately, as a linear list.
 *
 * **Read-only, and visibly so.** There is no checkout, no revert, no cherry-pick,
 * and no disabled control hinting at one — the absence of write actions is
 * deliberate and is part of the design rather than a gap in it
 * (`information-architecture.md` §5, Git view).
 *
 * **Nothing here polls.** History is the on-view tier: it is read when this
 * surface opens, when Refresh is pressed, and when somebody asks for more. There
 * is no interval and no scheduler observer behind it, and a guard test fails the
 * build if either appears (`information-architecture.md` §3).
 *
 * **No lanes.** A merge is a row with two parents, said in words on its detail.
 * The graph is 5b.
 */
export function History({
  project,
  layout,
  onBack,
}: {
  project: Project;
  layout: RepositoryLayout | null;
  onBack: () => void;
}) {
  const [opened, setOpened] = useState<CommitId | null>(null);

  // `Esc` goes back one level, everywhere (`information-architecture.md` §6
  // rule 2): out of a commit if one is open, out of History otherwise.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key !== 'Escape') return;
      if (opened) setOpened(null);
      else onBack();
    }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [opened, onBack]);

  const history = useInfiniteQuery({
    queryKey: ['git', 'history', project.id],
    queryFn: ({ pageParam }) => commands.gitHistory(project.id, pageParam),
    initialPageParam: null as CommitId | null,
    // `null` ends the paging. A page that could not be read has no next either,
    // so a failure stops the list rather than looping on it.
    getNextPageParam: (last) => (last.state === 'ready' ? last.next : null),
  });

  const pages = history.data?.pages ?? [];
  const first = pages[0];
  const commits: Commit[] = pages.flatMap((page) =>
    page.state === 'ready' ? page.commits : [],
  );

  return (
    <div className="flex min-w-0 flex-col gap-[var(--section-gap)]">
      <header className="flex flex-col gap-[var(--space-2)]">
        <button
          type="button"
          onClick={onBack}
          aria-label={`Back to ${project.name}`}
          className="t-ui flex w-fit cursor-default items-center gap-[var(--space-1)] text-ink-1 hover:text-ink-0"
        >
          <Icon name="back" />
          {project.name}
        </button>
        <h1 className="t-value-lg m-0 text-ink-0">History</h1>
        <RepositoryLine
          project={project}
          layout={layout}
          head={first?.state === 'ready' ? first.head : null}
        />
      </header>

      {opened ? (
        <CommitDetailPanel project={project} commit={opened} onClose={() => setOpened(null)} />
      ) : null}

      {history.isPending ? (
        <p className="t-ui m-0 text-ink-1">Reading history…</p>
      ) : history.isError ? (
        <p role="alert" className="t-body m-0 text-signal-danger">
          {describeUnknown(history.error)}
        </p>
      ) : first?.state === 'notARepository' ? (
        <Section label="">
          <Row mark={<span className="text-ink-3">○</span>} label="Not a repository" />
        </Section>
      ) : first?.state === 'unreadable' ? (
        <Section label="">
          <Row
            mark={<span className="text-signal-warn">◐</span>}
            label="History cannot be read"
            detail={first.detail}
          />
        </Section>
      ) : commits.length === 0 ? (
        <Section label="">
          <Row
            mark={<span className="text-ink-3">○</span>}
            label="No commits yet"
            detail="This repository has a branch but nothing on it."
          />
        </Section>
      ) : (
        <>
          <ul
            aria-label="Commits"
            className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
          >
            {commits.map((commit) => (
              <CommitRow
                key={commit.sha}
                project={project}
                commit={commit}
                opened={opened === commit.sha}
                onOpen={() => setOpened(opened === commit.sha ? null : commit.sha)}
              />
            ))}
          </ul>

          {first?.state === 'ready' && first.shallow ? (
            <p className="t-ui m-0 text-ink-2">
              This is a shallow copy, so the oldest commit here is where the clone stops — not
              where the history does.
            </p>
          ) : null}
        </>
      )}

      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        {history.hasNextPage ? (
          <Button
            onClick={() => void history.fetchNextPage()}
            disabled={history.isFetchingNextPage}
          >
            {history.isFetchingNextPage ? 'Loading…' : 'Load more'}
          </Button>
        ) : null}
        <Button onClick={() => void history.refetch()} disabled={history.isFetching}>
          {history.isFetching && !history.isFetchingNextPage ? 'Refreshing…' : 'Refresh'}
        </Button>
      </div>

      <p className="t-ui m-0 text-ink-1">
        Mira reads history and never changes it. There is nothing here that checks out, resets
        or rewrites a commit.
      </p>
    </div>
  );
}

/**
 * Which repository this history belongs to, and where its HEAD is.
 *
 * A package inside a monorepo names the repository above it, because that is
 * whose history this is. There is no per-package history and this line is what
 * says so ([ADR-0010](../../docs/adr/0010-monorepo-detection.md)).
 */
function RepositoryLine({
  project,
  layout,
  head,
}: {
  project: Project;
  layout: RepositoryLayout | null;
  head: Head | null;
}) {
  const repository =
    layout?.kind === 'package' ? repositoryName(layout.monorepoRoot) : project.name;

  return (
    <p className="t-ui m-0 flex flex-wrap items-center gap-[var(--space-3)] text-ink-1">
      {/* An unborn branch has nothing to name here. The empty list below says
          "No commits yet", and saying it twice would read as two facts. */}
      {head && head.kind !== 'unborn' ? (
        <span className="flex items-center gap-[var(--space-1)]">
          <Icon name="branch" />
          {headLabel(head)}
        </span>
      ) : null}
      <span className="text-ink-2">Repository · {repository}</span>
    </p>
  );
}

/** The last component of a path, which is what a repository is called. */
function repositoryName(root: string): string {
  const parts = root.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? root;
}

function headLabel(head: Head): string {
  switch (head.kind) {
    case 'branch':
      return head.name;
    case 'detached':
      return `Detached HEAD · ${head.sha}`;
    case 'unborn':
      return '';
  }
}

/**
 * One commit.
 *
 * Scannable without hover: the subject on its own line, then author, time and
 * abbreviated id. The icons mark what each value *is* so the eye can find the
 * author among three short strings; strip them out and every row still reads
 * (`design-system.md` §8, Icons).
 */
function CommitRow({
  project,
  commit,
  opened,
  onOpen,
}: {
  project: Project;
  commit: Commit;
  opened: boolean;
  onOpen: () => void;
}) {
  return (
    <li
      className={`flex items-start gap-[var(--space-3)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0 ${
        opened ? 'bg-ember-wash' : ''
      }`}
    >
      <span className={`mt-[3px] ${opened ? 'text-ember-bright' : 'text-ink-2'}`}>
        <Icon name="commit" />
      </span>

      <button
        type="button"
        onClick={onOpen}
        aria-expanded={opened}
        className="flex min-w-0 flex-1 cursor-default flex-col items-start gap-[var(--space-1)] text-left"
      >
        <span className="t-body w-full truncate text-ink-0" title={commit.subject}>
          {commit.subject || '(no message)'}
        </span>
        <span className="t-ui flex flex-wrap items-center gap-[var(--space-3)] text-ink-2">
          <span className="flex items-center gap-[var(--space-1)]">
            <Icon name="person" />
            {commit.author}
          </span>
          <span
            className="flex items-center gap-[var(--space-1)]"
            title={absoluteTime(commit.committedAt)}
          >
            <Icon name="clock" />
            {relativeTime(commit.committedAt)}
          </span>
          <span className="t-micro">{commit.shortSha}</span>
        </span>
      </button>

      <CopySha project={project} commit={commit.sha} form="short" label="Copy short SHA" />
    </li>
  );
}

/**
 * Copy a commit id.
 *
 * The interface names the **commit**, not the text. Mira resolves it in the
 * repository and copies what came back, so this button cannot be used to put a
 * string of the page's choosing on somebody's clipboard
 * (`security-and-privacy.md` §5, and a guard test).
 */
function CopySha({
  project,
  commit,
  form,
  label,
}: {
  project: Project;
  commit: CommitId;
  form: 'short' | 'full';
  label: string;
}) {
  const copy = useMutation({
    mutationFn: () => commands.gitCopyCommit(project.id, commit, form),
  });

  return (
    <span className="flex shrink-0 items-center gap-[var(--space-2)]">
      {copy.isSuccess ? (
        <span className="t-micro text-signal-ok" role="status">
          Copied
        </span>
      ) : null}
      {copy.isError ? (
        <span className="t-micro text-signal-danger" role="alert">
          {describeUnknown(copy.error)}
        </span>
      ) : null}
      <button
        type="button"
        onClick={() => copy.mutate()}
        aria-label={label}
        title={label}
        className="cursor-default rounded-sm p-[var(--space-1)] text-ink-2 hover:bg-ground-3 hover:text-ink-0"
      >
        <Icon name="copy" />
      </button>
    </span>
  );
}

/**
 * One commit, read-only.
 *
 * Subject, body, both spellings of the id, who wrote it and when, how many
 * parents it has, and how many paths it changed. **No diff** — what a commit
 * changed is 5b's read-only diff view, and half of one here would be the
 * speculative structure `roadmap.md` rule 8 exists to prevent.
 */
function CommitDetailPanel({
  project,
  commit,
  onClose,
}: {
  project: Project;
  commit: CommitId;
  onClose: () => void;
}) {
  const detail = useQuery({
    queryKey: ['git', 'commit', project.id, commit],
    queryFn: () => commands.gitCommit(project.id, commit),
  });

  if (detail.isPending) return <p className="t-ui m-0 text-ink-1">Reading the commit…</p>;
  if (detail.isError) {
    return (
      <p role="alert" className="t-body m-0 text-signal-danger">
        {describeUnknown(detail.error)}
      </p>
    );
  }

  const found = detail.data;
  if (found.state === 'unknown') {
    return (
      <Section label="Commit">
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
      <Section label="Commit">
        <Row
          mark={<span className="text-signal-warn">◐</span>}
          label="Cannot be read"
          detail={found.detail}
        />
      </Section>
    );
  }
  if (found.state === 'notARepository') return null;

  const it = found.commit;

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <div className="flex items-baseline justify-between gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">Commit</h2>
        <button
          type="button"
          onClick={onClose}
          className="t-ui cursor-default text-ink-2 hover:text-ink-0"
        >
          Close
        </button>
      </div>

      <div className="overflow-hidden rounded-md border border-line bg-ground-1">
        <div className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)]">
          <p className="t-body m-0 text-ink-0">{it.commit.subject || '(no message)'}</p>
          {it.body ? (
            <p className="t-ui m-0 whitespace-pre-wrap text-ink-1">{it.body}</p>
          ) : null}
        </div>

        <Row
          mark={<Icon name="person" />}
          label="Author"
          value={`${it.commit.author} · ${it.authorEmail}`}
        />
        <Row
          mark={<Icon name="clock" />}
          label="Committed"
          value={absoluteTime(it.commit.committedAt)}
          detail={relativeTime(it.commit.committedAt)}
        />
        <Row
          mark={<Icon name="commit" />}
          label="Commit"
          value={
            <span className="flex items-center gap-[var(--space-2)]">
              <span className="t-micro break-all">{it.commit.sha}</span>
              <CopySha
                project={project}
                commit={it.commit.sha}
                form="full"
                label="Copy full SHA"
              />
            </span>
          }
        />
        <Row
          label="Parents"
          value={parentage(it.parents)}
          detail={it.parents > 1 ? 'A merge. In 0.1 it is a row like any other.' : undefined}
        />
        <Row
          label="Changed files"
          value={
            it.changedFiles === null
              ? 'Not counted'
              : `${it.changedFiles} ${paths(it.changedFiles)}`
          }
          detail={
            it.changedFiles === null
              ? 'A merge changes different things depending on which parent you compare against, so there is no single number to show.'
              : undefined
          }
        />
        <Row label="Repository" value={project.name} />
      </div>
    </section>
  );
}

function parentage(parents: number): string {
  if (parents === 0) return 'None — the first commit';
  if (parents === 1) return '1';
  return `${parents} — a merge`;
}

function paths(count: number): string {
  return count === 1 ? 'path' : 'paths';
}
