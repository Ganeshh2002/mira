import { useInfiniteQuery, useMutation, useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

import type { Commit } from '../bindings/Commit';
import type { CommitGraph } from '../bindings/CommitGraph';
import type { CommitId } from '../bindings/CommitId';
import type { CommitPage } from '../bindings/CommitPage';
import type { FileSubject } from '../bindings/FileSubject';
import type { FilteredHistory } from '../bindings/FilteredHistory';
import type { GitRef } from '../bindings/GitRef';
import type { GraphRow } from '../bindings/GraphRow';
import type { Head } from '../bindings/Head';
import type { Project } from '../bindings/Project';
import type { RepositoryLayout } from '../bindings/RepositoryLayout';
import { Button } from '../components/Button';
import { Changes } from '../components/Changes';
import {
  FilterBar,
  NOTHING,
  asFilter,
  describeNarrowing,
  isNarrowed,
  type Narrowing,
} from '../components/FilterBar';
import { LaneGutter } from '../components/LaneGutter';
import { Icon } from '../components/Icon';
import { Row } from '../components/Row';
import { Section } from '../components/Section';
import { commands, describeUnknown } from '../lib/ipc';
import { absoluteTime, relativeTime } from '../lib/time';

/**
 * What happened lately, as a list — with the shape of it beside the list.
 *
 * **Read-only, and visibly so.** There is no checkout, revert, cherry-pick, merge
 * or reset here, and no disabled control hinting at one. The graph is a picture of
 * a repository, not a way to change it
 * ([ADR-0015](../../docs/adr/0015-graph-lanes.md)).
 *
 * **Nothing here polls.** History is the on-view tier: it is read when this
 * surface opens, when Refresh is pressed, and when somebody asks for more. There
 * is no interval behind it and no scheduler observer, and a guard test fails the
 * build if either appears (`information-architecture.md` §3).
 *
 * **Two modes, two costs.** Graph reads parents, lanes and reference labels; List
 * reads the commits alone. The toggle is a real choice rather than a cosmetic one
 * — and it is the deliberate end of the same behaviour that hides the gutter on a
 * narrow window.
 *
 * **Filtering is a third, costlier question.** Plain history walks twenty-five
 * commits per page; a filtered one may examine two thousand to find twenty-five,
 * so it is asked only when something is actually narrowed and it always says how
 * far it looked ([ADR-0018](../../docs/adr/0018-history-filters.md)).
 */
export function History({
  project,
  layout,
  onBack,
  initialCommit = null,
}: {
  project: Project;
  layout: RepositoryLayout | null;
  onBack: () => void;
  /**
   * A commit to open on arrival — how the working tree's file history hands a
   * reader back to the existing commit detail surface rather than growing one
   * of its own.
   */
  initialCommit?: CommitId | null;
}) {
  const [opened, setOpened] = useState<CommitId | null>(initialCommit);
  const [drawing, setDrawing] = useState(true);
  const [narrowing, setNarrowing] = useState<Narrowing>(NOTHING);

  const narrowed = isNarrowed(narrowing);
  const filter = asFilter(narrowing);

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
    // The filter is part of the key, so narrowing is a different question rather
    // than a mutation of the same one — and clearing it finds the plain history
    // already cached.
    queryKey: [
      'git',
      narrowed ? 'search' : drawing ? 'graph' : 'history',
      project.id,
      narrowed ? filter : null,
    ],
    // Annotated, because the three commands return three shapes of the same page
    // and the union is the thing this surface renders.
    queryFn: ({ pageParam }): Promise<CommitPage | CommitGraph | FilteredHistory> =>
      narrowed
        ? // The file travels in the cursor rather than the filter, because a
          // rename crossed mid-search changes which file the next page is about.
          commands.gitSearch(
            project.id,
            pageParam.file ? { ...filter, file: pageParam.file } : filter,
            pageParam.from,
          )
        : drawing
          ? commands.gitGraph(project.id, pageParam.from)
          : commands.gitHistory(project.id, pageParam.from),
    initialPageParam: { from: null as CommitId | null, file: null as FileSubject | null },
    // `null` ends the paging. A page that could not be read has no next either,
    // so a failure stops the list rather than looping on it.
    getNextPageParam: (last) => {
      if (last.state !== 'ready' || last.next === null) return null;

      return typeof last.next === 'string'
        ? { from: last.next, file: null }
        : { from: last.next.from, file: last.next.file };
    },
  });

  const pages = history.data?.pages ?? [];
  const first = pages[0];
  const rows: GraphRow[] = pages.flatMap((page) =>
    page.state === 'ready' ? asRows(page) : [],
  );
  const shape = first?.state === 'ready' ? first : null;
  const lanes = shape && 'lanes' in shape ? shape.lanes : 0;

  // How far the search got, which is a different fact from what it found. Summed
  // across pages, because "keep looking" spends the budget again.
  const newest = pages[pages.length - 1];
  const examined = pages.reduce(
    (total, page) => total + (page.state === 'ready' && 'scanned' in page ? page.scanned : 0),
    0,
  );
  const stopped = newest?.state === 'ready' && 'stopped' in newest ? newest.stopped : null;

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
        <div className="flex flex-wrap items-baseline justify-between gap-[var(--space-3)]">
          <h1 className="t-value-lg m-0 text-ink-0">History</h1>
          {/*
            A filtered history is a list of matches rather than a shape, so the
            graph is not offered while one is on. Clearing the filters brings it
            back, which is why the toggle's own state is left alone.
          */}
          <button
            type="button"
            hidden={narrowed}
            onClick={() => setDrawing(!drawing)}
            aria-pressed={drawing}
            className={`t-ui flex cursor-default items-center gap-[var(--space-2)] rounded-sm border px-[var(--space-3)] py-[var(--space-1)] transition-colors duration-[var(--motion-instant)] ${
              drawing
                ? 'border-ember-dim bg-ember-wash text-ember-bright'
                : 'border-line bg-ground-2 text-ink-1 hover:bg-ground-3'
            }`}
          >
            <Icon name="graph" />
            Graph
          </button>
        </div>
        <RepositoryLine
          project={project}
          layout={layout}
          head={first?.state === 'ready' ? first.head : null}
        />
        <FilterBar projectId={project.id} narrowing={narrowing} onChange={setNarrowing} />
        {/*
          What is narrowed, said in words — for a reader who cannot see which
          controls are lit (`design-system.md` §5).
        */}
        <p aria-live="polite" className="sr-only">
          {narrowed ? `Filtering by ${describeNarrowing(narrowing)}.` : 'No filters.'}
        </p>
      </header>

      {opened ? (
        <CommitDetailPanel
          project={project}
          commit={opened}
          onClose={() => setOpened(null)}
          onOpenCommit={setOpened}
        />
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
      ) : first?.state === 'unknown' ? (
        // Only a filtered read can answer this: the file a filter names is a
        // place in a change set, and a change set moves on.
        <Section label="">
          <Row
            mark={<span className="text-signal-warn">◐</span>}
            label="That file is no longer in this list"
            detail="Refresh to see what has changed since, then choose it again."
          />
        </Section>
      ) : first?.state === 'unreadable' ? (
        <Section label="">
          <Row
            mark={<span className="text-signal-warn">◐</span>}
            label="History cannot be read"
            detail={first.detail}
          />
        </Section>
      ) : rows.length === 0 ? (
        <Section label="">
          {/*
            The sentence this whole slice turns on. A search that ran out of
            budget has found nothing *yet*; a search that reached the end has
            found nothing at all. Saying the second when the first is true would
            be a lie about the repository.
          */}
          {narrowed ? (
            <Row
              mark={<span className="text-ink-3">○</span>}
              label={stopped?.state === 'budget' ? 'No match yet' : 'No matching commits'}
              detail={
                stopped?.state === 'budget'
                  ? `Nothing matched in the ${examined} commits examined. There may be more further back.`
                  : `Nothing in this history matches ${describeNarrowing(narrowing)}.`
              }
            />
          ) : (
            <Row
              mark={<span className="text-ink-3">○</span>}
              label="No commits yet"
              detail="This repository has a branch but nothing on it."
            />
          )}
        </Section>
      ) : (
        <>
          <CommitList
            project={project}
            rows={rows}
            lanes={drawing ? lanes : 0}
            opened={opened}
            onOpen={(sha) => setOpened(opened === sha ? null : sha)}
          />

          {shape && 'collapsed' in shape && shape.collapsed ? (
            <p className="t-ui m-0 text-ink-2">
              More branches meet here than the graph draws, so everything past the eighth lane
              shares the last line. Every commit is still listed.
            </p>
          ) : null}
          {shape && 'refsTruncated' in shape && shape.refsTruncated ? (
            <p className="t-ui m-0 text-ink-2">
              This repository has more references than Mira reads at once, so a branch or tag
              label may be missing from a row that has one.
            </p>
          ) : null}
          {narrowed ? (
            <p className="t-ui m-0 text-ink-1">
              {rows.length} {rows.length === 1 ? 'commit' : 'commits'} matching{' '}
              {describeNarrowing(narrowing)}, of {examined} examined.
            </p>
          ) : null}
          {narrowed && stopped?.state === 'budget' ? (
            <p className="t-ui m-0 text-ink-2">
              Stopped after examining {examined} commits, so this is what matched so far rather
              than everything that matches.
            </p>
          ) : null}
          {narrowed && stopped?.state === 'renameLost' ? (
            <p className="t-ui m-0 text-ink-2">
              This file was renamed in a commit that changed more paths than Mira reads at once,
              so the search ends at <span className="t-micro">{stopped.path}</span>.
            </p>
          ) : null}
          {shape?.shallow ? (
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
            {history.isFetchingNextPage
              ? narrowed
                ? 'Looking further back…'
                : 'Loading…'
              : narrowed
                ? 'Keep looking'
                : 'Load more'}
          </Button>
        ) : null}
        <Button onClick={() => void history.refetch()} disabled={history.isFetching}>
          {history.isFetching && !history.isFetchingNextPage ? 'Refreshing…' : 'Refresh'}
        </Button>
      </div>

      <p className="t-ui m-0 text-ink-1">
        Mira draws history and never changes it. There is nothing here that checks out, merges,
        rebases, resets or rewrites a commit.
      </p>
    </div>
  );
}

/**
 * A page from either command, as one row shape.
 *
 * The List mode's page has no lanes, parents or labels — so it becomes a row with
 * none, and one list renders both. That is what keeps the two modes a difference
 * in *cost* rather than a second surface to maintain.
 */
function asRows(page: { commits?: Commit[]; rows?: GraphRow[] }): GraphRow[] {
  if (page.rows) return page.rows;

  return (page.commits ?? []).map((commit) => ({
    commit,
    parents: [],
    lane: 0,
    kind: 'normal' as const,
    refs: [],
    edges: [],
    continuing: [],
  }));
}

/**
 * The commit list.
 *
 * `↑` and `↓` move between rows, which is what `information-architecture.md` §7
 * asks of every list in the product. Tab still reaches every row; the arrow keys
 * are the faster path, not the only one.
 */
function CommitList({
  project,
  rows,
  lanes,
  opened,
  onOpen,
}: {
  project: Project;
  rows: GraphRow[];
  lanes: number;
  opened: CommitId | null;
  onOpen: (sha: CommitId) => void;
}) {
  const list = useRef<HTMLUListElement>(null);

  function onKeyDown(event: React.KeyboardEvent<HTMLUListElement>) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;

    const buttons = Array.from(
      list.current?.querySelectorAll<HTMLButtonElement>('button[data-commit]') ?? [],
    );
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (at === -1) return;

    const moving = buttons[event.key === 'ArrowDown' ? at + 1 : at - 1];
    if (!moving) return;

    event.preventDefault();
    moving.focus();
  }

  return (
    <ul
      ref={list}
      aria-label="Commits"
      onKeyDown={onKeyDown}
      className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
    >
      {rows.map((row, index) => (
        <CommitRow
          key={row.commit.sha}
          project={project}
          row={row}
          // The lanes the row above left open, so its lines meet this one's.
          above={index === 0 ? [] : (rows[index - 1]?.continuing ?? [])}
          lanes={lanes}
          opened={opened === row.commit.sha}
          onOpen={() => onOpen(row.commit.sha)}
        />
      ))}
    </ul>
  );
}

/**
 * One commit.
 *
 * Scannable without hover and without the gutter: the subject on its own line,
 * then what kind of commit it is, who wrote it, when, and the abbreviated id. The
 * graph adds a picture of the same relationships; it never carries one on its own,
 * which is why hiding it on a narrow window loses width and nothing else.
 */
function CommitRow({
  project,
  row,
  above,
  lanes,
  opened,
  onOpen,
}: {
  project: Project;
  row: GraphRow;
  above: number[];
  lanes: number;
  opened: boolean;
  onOpen: () => void;
}) {
  const { commit } = row;

  return (
    <li
      className={`flex items-stretch gap-[var(--space-3)] border-b border-line pr-[var(--space-3)] last:border-b-0 ${
        opened ? 'bg-ember-wash' : ''
      }`}
    >
      {/*
        The gutter is hidden below the `sm` breakpoint rather than reflowed. A
        narrow window keeps the list, which carries every fact the picture does —
        degrading by leaving out the decoration, never the content.
      */}
      {lanes > 0 ? (
        <span className="hidden shrink-0 items-center pl-[var(--space-2)] sm:flex">
          <LaneGutter row={row} above={above} lanes={lanes} />
        </span>
      ) : (
        <span className="flex shrink-0 items-center pl-[var(--space-3)] text-ink-2">
          <Icon name={row.kind === 'merge' ? 'merge' : 'commit'} />
        </span>
      )}

      <button
        type="button"
        data-commit={commit.sha}
        onClick={onOpen}
        aria-expanded={opened}
        className="flex min-h-[var(--graph-row)] min-w-0 flex-1 cursor-default flex-col items-start justify-center gap-[var(--space-1)] py-[var(--space-2)] text-left"
      >
        <span className="flex w-full min-w-0 items-center gap-[var(--space-2)]">
          <span className="t-body min-w-0 truncate text-ink-0" title={commit.subject}>
            {commit.subject || '(no message)'}
          </span>
          <RefLabels refs={row.refs} />
        </span>
        <span className="t-ui flex flex-wrap items-center gap-[var(--space-3)] text-ink-2">
          {/*
            The word, not only the shape. A merge stays distinguishable with the
            gutter hidden, in a screen reader, and on a monochrome display
            (`design-system.md` §5).
          */}
          {row.kind !== 'normal' ? (
            <span className="flex items-center gap-[var(--space-1)] text-ink-1">
              <Icon name={row.kind === 'merge' ? 'merge' : 'commit'} />
              {row.kind === 'merge' ? `Merge of ${row.parents.length} parents` : 'First commit'}
            </span>
          ) : null}
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

      <span className="flex shrink-0 items-center">
        <CopySha project={project} commit={commit.sha} form="short" label="Copy short SHA" />
      </span>
    </li>
  );
}

/**
 * Branch and tag labels on a row.
 *
 * Each says what it is in its title, because `main` and `v1.0` look alike and mean
 * different things — and because a chip that only differed by colour would be no
 * label at all (`design-system.md` §5).
 */
function RefLabels({ refs }: { refs: GitRef[] }) {
  if (refs.length === 0) return null;

  return (
    <span className="flex min-w-0 shrink flex-wrap items-center gap-[var(--space-1)]">
      {refs.map((found) => (
        <span
          key={`${found.kind}-${found.name}`}
          title={`${describeRef(found.kind)}: ${found.name}`}
          className={`t-micro flex max-w-[14ch] items-center gap-[var(--space-1)] rounded-sm px-[var(--space-1)] ${
            found.kind === 'head' ? 'bg-ember-wash text-ember-bright' : 'bg-ground-3 text-ink-1'
          }`}
        >
          <Icon name={found.kind === 'tag' ? 'tag' : 'branch'} />
          <span className="truncate">{found.name}</span>
        </span>
      ))}
    </span>
  );
}

function describeRef(kind: GitRef['kind']): string {
  switch (kind) {
    case 'head':
      return 'Where HEAD is';
    case 'branch':
      return 'Branch';
    case 'remote':
      return 'Remote branch, as of your last fetch';
    case 'tag':
      return 'Tag';
  }
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
 * Subject, body, both spellings of the id, who wrote it and when, how many parents
 * it has — and, below that, what it changed.
 *
 * The changed files sit **inside** this panel rather than opening a further level,
 * and a file's patch opens inside its own row. History → commit → files → patch is
 * four things to read and one place to be, which is what keeps
 * `information-architecture.md` §6's depth cap true rather than merely obeyed.
 */
function CommitDetailPanel({
  project,
  commit,
  onClose,
  onOpenCommit,
}: {
  project: Project;
  commit: CommitId;
  onClose: () => void;
  /** Follow a file's history to another commit, which opens here. */
  onOpenCommit: (commit: CommitId) => void;
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
          mark={it.parents > 1 ? <Icon name="merge" /> : undefined}
          label="Parents"
          value={parentage(it.parents)}
          detail={
            it.parents > 1 ? 'A merge. In the graph it is a row with two lines.' : undefined
          }
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
              ? 'A merge changes different things depending on which parent you compare against. The list below compares with the first.'
              : undefined
          }
        />
        <Row label="Repository" value={project.name} />
      </div>

      <Changes
        projectId={project.id}
        scope={{ kind: 'commit', commit }}
        label="Changed files"
        onOpenCommit={onOpenCommit}
      />
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
