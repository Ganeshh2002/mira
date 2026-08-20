import type { AppKind } from '../bindings/AppKind';
import type { AppReport } from '../bindings/AppReport';

/**
 * The applications a workspace works with.
 *
 * Two different facts, kept apart on purpose. Which *kinds* belong to this
 * workspace is something the person stated and Mira stored; whether one is
 * *installed* is read fresh on whichever machine Mira is running on. That split
 * is what lets a row say "Editor · Not installed" — the association survives a
 * move to a machine without VS Code, rather than being quietly dropped.
 *
 * There is nothing to click. Launching an application is a later slice with its
 * own design, and a disabled button hinting at it would be worse than its
 * absence (`information-architecture.md` §5).
 */
export function ContextPanel({
  kinds,
  available,
  onToggle,
}: {
  kinds: AppKind[];
  available: AppReport[];
  onToggle: (kind: AppKind, wanted: boolean) => void;
}) {
  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <h2 className="t-label text-ink-1">Context</h2>

      <ul
        aria-label="Context"
        className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
      >
        {available.map((report) => {
          const included = kinds.includes(report.kind);

          return (
            <li
              key={report.kind}
              className="flex min-h-[var(--row-height)] items-center gap-[var(--space-3)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0"
            >
              <span
                aria-hidden="true"
                className={included ? 'text-ember-bright' : 'text-ink-1'}
              >
                {included ? '●' : '○'}
              </span>

              <span className="t-ui shrink-0 text-ink-1">{label(report.kind)}</span>

              <span className="t-value ml-auto min-w-0 truncate text-ink-0">
                {report.presence.state === 'available' ? (
                  report.presence.name
                ) : (
                  <span className="text-ink-1">Not installed</span>
                )}
              </span>

              <label className="t-ui flex shrink-0 cursor-default items-center gap-[var(--space-2)] text-ink-1">
                <input
                  type="checkbox"
                  checked={included}
                  onChange={(event) => onToggle(report.kind, event.target.checked)}
                />
                <span className="sr-only sm:not-sr-only">Part of this workspace</span>
              </label>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * The word for a kind.
 *
 * Interface copy lives here rather than coming over the wire: the backend sends
 * the variant, and nothing in this file names a specific application.
 */
function label(kind: AppKind): string {
  switch (kind) {
    case 'editor':
      return 'Editor';
    case 'terminal':
      return 'Terminal';
    case 'browser':
      return 'Browser';
  }
}
