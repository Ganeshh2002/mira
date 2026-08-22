/**
 * The icon set.
 *
 * One set, 16 px, 1.5 px stroke, monochrome, inheriting `currentColor`. No filled
 * icons, no brand marks, no emoji in product chrome (`design-system.md` §8).
 *
 * Icons here are **companions to words, not replacements for them.** Every row in
 * the History surface reads correctly with the icons stripped out — they mark
 * what a value *is* so the eye can find the author among three short strings,
 * which is the one job an icon does better than a label. The single icon that
 * carries an action on its own, Copy, has an accessible name and a tooltip
 * (`design-system.md` §8, Button).
 */

export type IconName =
  | 'branch'
  | 'commit'
  | 'merge'
  | 'tag'
  | 'graph'
  | 'rename'
  | 'binary'
  | 'trace'
  | 'filter'
  | 'search'
  | 'person'
  | 'clock'
  | 'copy'
  | 'awake'
  | 'back'
  | 'service'
  | 'add'
  | 'remove';

/** The path geometry for each icon, on a 16×16 grid. */
const PATHS: Record<IconName, React.ReactNode> = {
  // A branch leaving a trunk: the shape Git itself uses.
  branch: (
    <>
      <circle cx="4.5" cy="3.5" r="1.75" />
      <circle cx="4.5" cy="12.5" r="1.75" />
      <circle cx="11.5" cy="4.5" r="1.75" />
      <path d="M4.5 5.25v5.5M11.5 6.25v0.5a3 3 0 0 1-3 3H6" />
    </>
  ),
  // A commit on a line — the row marker in the history list.
  commit: (
    <>
      <circle cx="8" cy="8" r="2.75" />
      <path d="M1.5 8h3.75M10.75 8h3.75" />
    </>
  ),
  // Two lines becoming one: what a merge commit is.
  merge: (
    <>
      <circle cx="4" cy="3.5" r="1.75" />
      <circle cx="12" cy="3.5" r="1.75" />
      <circle cx="8" cy="12.5" r="1.75" />
      <path d="M4 5.25v1.25a3 3 0 0 0 3 3h0.4M12 5.25v1.25a3 3 0 0 1-3 3h-0.4" />
    </>
  ),
  tag: (
    <>
      <path d="M2.5 7.5V3a0.5 0.5 0 0 1 0.5-0.5h4.5l6 6a1 1 0 0 1 0 1.4l-3.6 3.6a1 1 0 0 1-1.4 0l-6-6z" />
      <circle cx="5.25" cy="5.25" r="0.9" />
    </>
  ),
  // Lanes: what the gutter draws, as a control for showing it.
  graph: (
    <>
      <path d="M4 2.5v11M11 5.5v8" />
      <circle cx="4" cy="6" r="1.5" />
      <circle cx="11" cy="10.5" r="1.5" />
      <path d="M5.5 6h1.5a3 3 0 0 1 3 3v0.6" />
    </>
  ),
  // An arrow turning into a new place: what a rename or a copy is.
  rename: (
    <>
      <path d="M2.5 4.5h6a3.5 3.5 0 0 1 3.5 3.5v3.5" />
      <path d="M9.75 13.25L12 11l2.25 2.25" />
      <path d="M2.5 4.5L4.75 2.25M2.5 4.5l2.25 2.25" />
    </>
  ),
  // Not text: a file Mira reports the size of rather than decoding.
  binary: (
    <>
      <path d="M3.5 2.5h6l3 3v8a0.5 0.5 0 0 1-0.5 0.5H3.5a0.5 0.5 0 0 1-0.5-0.5v-11a0.5 0.5 0 0 1 0.5-0.5z" />
      <path d="M9.25 2.5v3.25h3.25" />
      <path d="M5.5 9.5h1.5v2.5H5.5zM9 9.5h1.5v2.5H9z" />
    </>
  ),
  // A file with a line of history running back from it.
  trace: (
    <>
      <path d="M9.5 2.5H4.5a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1V5.5z" />
      <path d="M9.25 2.5v3.25h3.25" />
      <path d="M5.75 8.25h4.5M5.75 10.75h3" />
    </>
  ),
  // A funnel: what a filter does to a list.
  filter: (
    <>
      <path d="M2 3.25h12l-4.5 5.25v4.25l-3 1.75V8.5z" />
    </>
  ),
  search: (
    <>
      <circle cx="7" cy="7" r="4.25" />
      <path d="M10.25 10.25L14 14" />
    </>
  ),
  person: (
    <>
      <circle cx="8" cy="5.5" r="2.5" />
      <path d="M3 13.5a5 5 0 0 1 10 0" />
    </>
  ),
  clock: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 4.5V8l2.5 1.75" />
    </>
  ),
  copy: (
    <>
      <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" />
      <path d="M10.5 3.5a1.5 1.5 0 0 0-1.5-1.5H4a1.5 1.5 0 0 0-1.5 1.5V9a1.5 1.5 0 0 0 1.5 1.5" />
    </>
  ),
  // Sleep prevented: a sun, because the machine is being asked to stay up.
  awake: (
    <>
      <circle cx="8" cy="8" r="3.25" />
      <path d="M8 1v1.75M8 13.25V15M15 8h-1.75M2.75 8H1M12.95 3.05l-1.24 1.24M4.29 11.71l-1.24 1.24M12.95 12.95l-1.24-1.24M4.29 4.29L3.05 3.05" />
    </>
  ),
  back: <path d="M10 3L5 8l5 5" />,
  // A server with a light on it: what a listening service is.
  service: (
    <>
      <rect x="2.5" y="3" width="11" height="4.5" rx="1" />
      <rect x="2.5" y="8.5" width="11" height="4.5" rx="1" />
      <path d="M5 5.25h0.01M5 10.75h0.01" />
    </>
  ),
  add: <path d="M8 3.5v9M3.5 8h9" />,
  remove: <path d="M4 4l8 8M12 4l-8 8" />,
};

/**
 * One icon.
 *
 * `label` gives it an accessible name where it stands alone; without one it is
 * decoration beside text and is hidden from assistive technology rather than read
 * out twice.
 */
export function Icon({
  name,
  label,
  className = '',
}: {
  name: IconName;
  label?: string | undefined;
  className?: string;
}) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={`shrink-0 ${className}`}
      role={label ? 'img' : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      focusable="false"
    >
      {PATHS[name]}
    </svg>
  );
}
