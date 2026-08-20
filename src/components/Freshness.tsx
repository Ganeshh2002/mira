import { relativeTime } from '../lib/time';

/**
 * When a reading was taken.
 *
 * Shown wherever observed data is, because observed data is only as good as its
 * age and the interface must never present a stale answer as a current one
 * (slice brief §8). "Updated just now" and "Updated 3 min ago" are the same
 * sentence; only one of them invites you to trust it.
 */
export function Freshness({ observedAt }: { observedAt: number | null }) {
  if (observedAt === null) {
    return <span className="t-ui text-ink-1">Not observed yet</span>;
  }

  return <span className="t-ui text-ink-1">Updated {relativeTime(observedAt)}</span>;
}
