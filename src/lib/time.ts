/**
 * How long ago something happened, in the form the header shows.
 *
 * Coarse on purpose: "2 h ago" is what a person wants when orienting themselves
 * in a project, and a ticking "1 h 58 m" would be motion for its own sake
 * (design-system §6). The exact timestamp is available on hover.
 */
export function relativeTime(epochSeconds: number, now = Date.now()): string {
  const seconds = Math.round(now / 1000 - epochSeconds);

  if (seconds < 0) return 'just now';
  if (seconds < 60) return 'just now';

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} min ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;

  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} d ago`;

  const months = Math.floor(days / 30);
  if (months < 12) return `${months} mo ago`;

  return `${Math.floor(months / 12)} y ago`;
}

/** The full timestamp, for the title a person can hover to see. */
export function absoluteTime(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleString();
}
