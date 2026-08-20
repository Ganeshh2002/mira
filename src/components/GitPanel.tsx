import type { Commit } from '../bindings/Commit';
import type { GitOverview } from '../bindings/GitOverview';
import type { Head } from '../bindings/Head';
import type { Upstream } from '../bindings/Upstream';
import { absoluteTime, relativeTime } from '../lib/time';
import { Row } from './Row';
import { Section } from './Section';

/**
 * Read-only Git context.
 *
 * There are no write actions here and no disabled ones either: the absence of a
 * commit button is deliberate and visible, not a hint at something coming
 * (`information-architecture.md` §5, "Git view").
 */
export function GitPanel({ git }: { git: GitOverview }) {
  if (git.state === 'notARepository') {
    return (
      <Section label="Git">
        <Row mark={<span className="text-ink-3">○</span>} label="Not a repository" />
      </Section>
    );
  }

  if (git.state === 'unreadable') {
    return (
      <Section label="Git">
        <Row
          mark={<span className="text-signal-warn">◐</span>}
          label="Cannot be read"
          detail={git.detail}
        />
      </Section>
    );
  }

  return (
    <Section label="Git">
      <Row
        mark={<HeadMark head={git.head} />}
        label={headLabel(git.head)}
        value={headValue(git.head)}
      />
      <Row
        mark={
          git.clean ? (
            <span className="text-signal-ok">✓</span>
          ) : (
            <span className="text-ember">●</span>
          )
        }
        label={git.clean ? 'Clean' : `${git.changed} changed`}
      />
      {git.upstream ? <UpstreamRows upstream={git.upstream} /> : null}
      <LastCommit commit={git.lastCommit} />
    </Section>
  );
}

function HeadMark({ head }: { head: Head }) {
  if (head.kind === 'unborn') return <span className="text-ink-3">○</span>;
  return <span className="text-ember-bright">●</span>;
}

function headLabel(head: Head): string {
  switch (head.kind) {
    case 'branch':
      return 'Branch';
    case 'detached':
      return 'Detached HEAD';
    case 'unborn':
      return 'No commits yet';
  }
}

function headValue(head: Head) {
  switch (head.kind) {
    case 'branch':
      return head.name;
    case 'detached':
      return head.sha;
    case 'unborn':
      return undefined;
  }
}

/**
 * Ahead and behind, with the staleness said out loud.
 *
 * Mira never fetches (`prd.md` FR-5.2), so these counts are as old as the user's
 * last fetch. FR-5.3 asks for that to be labelled rather than left to be
 * discovered, which is the honest way to solve it without a background network
 * request nobody asked for.
 */
function UpstreamRows({ upstream }: { upstream: Upstream }) {
  return (
    <Row
      label={upstream.name}
      value={
        <span className="flex items-center gap-[var(--space-3)]">
          <span className={upstream.ahead > 0 ? 'text-ember-bright' : 'text-ink-2'}>
            ↑ {upstream.ahead} ahead
          </span>
          <span className={upstream.behind > 0 ? 'text-signal-warn' : 'text-ink-2'}>
            ↓ {upstream.behind} behind
          </span>
        </span>
      }
      detail="As of last fetch. Mira does not fetch on its own."
    />
  );
}

function LastCommit({ commit }: { commit: Commit | null }) {
  if (!commit) return null;

  return (
    <div className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0">
      <p className="t-body m-0 truncate text-ink-0" title={commit.subject}>
        {commit.subject}
      </p>
      <p className="t-ui m-0 text-ink-2">
        <span>{commit.author}</span>
        {' · '}
        <span title={absoluteTime(commit.committedAt)}>{relativeTime(commit.committedAt)}</span>
        {' · '}
        <span className="t-micro">{commit.shortSha}</span>
      </p>
    </div>
  );
}
