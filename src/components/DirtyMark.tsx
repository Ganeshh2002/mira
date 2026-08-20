import type { GitOverview } from '../bindings/GitOverview';

/**
 * The project list's status dot (`design-system.md` §5).
 *
 * Colour is never the only channel: each state has its own glyph and its own
 * accessible name, so the mark survives colour blindness, a bad monitor, and a
 * screen reader. "Not observed yet" is a real state and says so, rather than
 * borrowing the clean mark and quietly claiming something Mira does not know.
 */
export function DirtyMark({ git }: { git: GitOverview | null | undefined }) {
  const { glyph, name, className } = describe(git);

  return (
    <span role="img" aria-label={name} title={name} className={`select-none ${className}`}>
      {glyph}
    </span>
  );
}

function describe(git: GitOverview | null | undefined) {
  if (!git) {
    return { glyph: '·', name: 'Not observed yet', className: 'text-ink-1' };
  }

  switch (git.state) {
    case 'notARepository':
      return { glyph: '○', name: 'Not a repository', className: 'text-ink-1' };
    case 'unreadable':
      return { glyph: '◐', name: 'Cannot be read', className: 'text-signal-warn' };
    case 'ready':
      return git.clean
        ? { glyph: '✓', name: 'Clean', className: 'text-signal-ok' }
        : {
            glyph: '●',
            name: `${git.changed} changed`,
            className: 'text-ember-bright',
          };
  }
}
