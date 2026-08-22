import { useQuery } from '@tanstack/react-query';
import { useEffect, useId, useState } from 'react';

import type { AuthorCount } from '../bindings/AuthorCount';
import type { FileSubject } from '../bindings/FileSubject';
import type { HistoryFilter } from '../bindings/HistoryFilter';
import type { RefTip } from '../bindings/RefTip';
import { Icon } from './Icon';
import { Menu } from './Menu';
import { commands } from '../lib/ipc';

/**
 * What the reader has narrowed the history to.
 *
 * Two halves, deliberately: the **names** live here so the bar can be read, and
 * the **values** go to Mira through [`asFilter`]. A branch is chosen by its name
 * and asked for by its tip; a file is chosen by its path and asked for by its
 * place in a change set. Nothing the reader can see is what gets sent.
 */
export type Narrowing = {
  branch: RefTip | null;
  author: AuthorCount | null;
  /** A file Mira offered, with the path kept for the label only. */
  file: { subject: FileSubject; path: string } | null;
  /** Free text, matched against the subject line. */
  subject: string;
};

/** No filter at all — the default history view. */
export const NOTHING: Narrowing = { branch: null, author: null, file: null, subject: '' };

/** Whether anything is narrowed, and so whether to ask the costlier question. */
export function isNarrowed(narrowing: Narrowing): boolean {
  return Boolean(
    narrowing.branch || narrowing.author || narrowing.file || narrowing.subject.trim(),
  );
}

/**
 * The filter as Mira takes it.
 *
 * This is the whole boundary in one function: a `RefTip` becomes its `tip`, a
 * chosen file becomes its `subject`, and the two text fields become trimmed
 * strings. No name, no path, and nothing shaped like an argument
 * ([ADR-0018](../../docs/adr/0018-history-filters.md)).
 */
export function asFilter(narrowing: Narrowing): HistoryFilter {
  return {
    branch: narrowing.branch?.tip ?? null,
    author: narrowing.author?.name ?? null,
    subject: narrowing.subject.trim() || null,
    file: narrowing.file?.subject ?? null,
  };
}

/** The filter, said out loud — for the live region and for the cleared state. */
export function describeNarrowing(narrowing: Narrowing): string {
  const parts = [
    narrowing.branch && `branch ${narrowing.branch.name}`,
    narrowing.author && `author ${narrowing.author.name}`,
    narrowing.file && `file ${narrowing.file.path}`,
    narrowing.subject.trim() && `“${narrowing.subject.trim()}” in the subject`,
  ].filter(Boolean) as string[];

  return parts.join(', ');
}

/**
 * `[ Branch ▾ ] [ Author ▾ ] [ File ▾ ] [ Search ]`
 *
 * **Every menu is a list Mira produced.** Branches come from `git.refs`, authors
 * from the commits within one scan budget, and files from the working tree's
 * bounded change list. There is no field here that accepts a name and hands it to
 * Git, because there is no field here that accepts a name at all.
 *
 * **Composable, and clearable.** Filters narrow together; Clear returns the
 * surface to ordinary paginated history rather than to an empty search.
 *
 * Icons are here to make the bar scannable, never to carry the meaning: each one
 * sits beside its own word (`design-system.md` §5).
 */
export function FilterBar({
  projectId,
  narrowing,
  onChange,
}: {
  projectId: number;
  narrowing: Narrowing;
  onChange: (narrowing: Narrowing) => void;
}) {
  const refs = useQuery({
    queryKey: ['git', 'refs', projectId],
    queryFn: () => commands.gitRefs(projectId),
  });
  const authors = useQuery({
    queryKey: ['git', 'authors', projectId],
    queryFn: () => commands.gitAuthors(projectId),
  });
  // The working tree's change list: a bounded set of files Mira already named,
  // which is what makes each one askable-about without a path being typed.
  const files = useQuery({
    queryKey: ['git', 'changes', projectId, { kind: 'workingTree' }],
    queryFn: () => commands.gitChanges(projectId, { kind: 'workingTree' }),
  });

  const branches = refs.data?.state === 'ready' ? refs.data.refs : [];
  const people = authors.data?.state === 'ready' ? authors.data.authors : [];
  const changed = files.data?.state === 'ready' ? files.data.files : [];

  const narrowed = isNarrowed(narrowing);

  return (
    <div
      role="search"
      aria-label="Filter history"
      className="flex flex-wrap items-center gap-[var(--space-2)]"
    >
      <span className="t-micro flex items-center gap-[var(--space-1)] text-ink-2">
        <Icon name="filter" />
        Filter
      </span>

      <Menu
        icon="branch"
        label="Branch"
        chosen={narrowing.branch?.name ?? null}
        empty={
          refs.data?.state === 'ready'
            ? 'This repository has no branches or tags.'
            : 'No branches to choose from.'
        }
        options={branches.map((tip) => ({
          key: `${tip.kind}:${tip.name}`,
          label: tip.name,
          note: KIND_WORDS[tip.kind],
          chosen: narrowing.branch?.tip === tip.tip && narrowing.branch?.name === tip.name,
          choose: () => onChange({ ...narrowing, branch: tip }),
        }))}
        onClear={() => onChange({ ...narrowing, branch: null })}
        footer={
          refs.data?.state === 'ready' && refs.data.truncated
            ? 'This repository has more references than Mira reads at once.'
            : null
        }
      />

      <Menu
        icon="person"
        label="Author"
        chosen={narrowing.author?.name ?? null}
        empty="No authors to choose from."
        options={people.map((author) => ({
          key: author.name,
          label: author.name,
          note: `${author.commits}`,
          chosen: narrowing.author?.name === author.name,
          choose: () => onChange({ ...narrowing, author }),
        }))}
        onClear={() => onChange({ ...narrowing, author: null })}
        footer={
          authors.data?.state === 'ready' && authors.data.stopped.state === 'budget'
            ? `Authors of the last ${authors.data.scanned} commits. Somebody further back may be missing.`
            : null
        }
      />

      <Menu
        icon="trace"
        label="File"
        chosen={narrowing.file?.path ?? null}
        empty="Nothing is changed in the working tree, so there is no file here to pick. A commit’s changed files can be traced from the commit itself."
        options={changed.map((change) => ({
          key: `${change.at}`,
          label: change.path,
          note: null,
          chosen: narrowing.file?.path === change.path,
          choose: () =>
            onChange({
              ...narrowing,
              file: {
                // The file is named by where it sits in this list, never by the
                // path beside it — the path is the label and nothing else.
                subject: { scope: { kind: 'workingTree' }, at: change.at, before: false },
                path: change.path,
              },
            }),
        }))}
        onClear={() => onChange({ ...narrowing, file: null })}
        footer={null}
      />

      <SubjectSearch
        value={narrowing.subject}
        onSearch={(subject) => onChange({ ...narrowing, subject })}
      />

      {narrowed ? (
        <button
          type="button"
          onClick={() => onChange(NOTHING)}
          className="t-ui cursor-default rounded-sm border border-line bg-ground-2 px-[var(--space-3)] py-[var(--space-1)] text-ink-1 transition-colors duration-[var(--motion-instant)] hover:bg-ground-3"
        >
          Clear filters
        </button>
      ) : null}
    </div>
  );
}

/** What a reference is, in a word, so the kind is never colour alone. */
const KIND_WORDS: Record<RefTip['kind'], string> = {
  head: 'head',
  branch: 'branch',
  remote: 'remote',
  tag: 'tag',
};

/**
 * The free-text half.
 *
 * Submitted rather than typed-through: a search costs a bounded walk, and a walk
 * per keystroke would be a walk per keystroke. What it matches is said in the
 * field's own description, because "search" means five different things in five
 * different tools.
 */
function SubjectSearch({
  value,
  onSearch,
}: {
  value: string;
  onSearch: (subject: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  const hint = useId();

  // Cleared from outside — the Clear filters button — empties the field too.
  useEffect(() => setDraft(value), [value]);

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        onSearch(draft);
      }}
      className="flex items-center gap-[var(--space-1)]"
    >
      <label
        htmlFor="history-subject-search"
        className="t-micro flex items-center gap-[var(--space-1)] text-ink-2"
      >
        <Icon name="search" />
        Search
      </label>
      <input
        id="history-subject-search"
        type="search"
        value={draft}
        aria-describedby={hint}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="subject line"
        className="t-ui w-[var(--field-width)] max-w-[40vw] rounded-sm border border-line bg-ground-2 px-[var(--space-2)] py-[var(--space-1)] text-ink-0 placeholder:text-ink-3"
      />
      <p id={hint} className="sr-only">
        Matches part of a commit’s subject line, ignoring case. Not a pattern: wildcards and
        regular expressions are searched for literally. Press Enter to search.
      </p>
    </form>
  );
}
