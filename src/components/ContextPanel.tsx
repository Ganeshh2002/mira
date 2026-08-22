import type { AppKind } from '../bindings/AppKind';
import type { AppPreference } from '../bindings/AppPreference';
import type { AppReport } from '../bindings/AppReport';
import { AppChooser } from './AppChooser';
import { appKindLabel } from '../lib/applications';

/**
 * The applications a workspace works with.
 *
 * Two different facts, kept apart on purpose. Which *kinds* belong to this
 * workspace is something the person stated and Mira stored; whether one is
 * *installed* is read fresh on whichever machine Mira is running on. That split
 * is what lets a row say "Editor · Not installed" — the association survives a
 * move to a machine without that editor, rather than being quietly dropped.
 *
 * **Which one** is the third fact, and it is this workspace's alone. The chooser
 * on each row offers what Mira looks for on this platform, marked with what is
 * actually here; leaving it on Automatic is what every workspace did before
 * anybody chose anything. A choice is stored against this workspace id, so
 * choosing one editor here changes nothing for the workspace beside it
 * ([ADR-0019](../../docs/adr/0019-application-preferences.md)).
 *
 * Opening one is still the "Open with" row above. Keeping the association, the
 * choice and the action apart is what lets a workspace say it works with an
 * editor on a machine that has none.
 */
export function ContextPanel({
  workspaceId,
  kinds,
  preferences,
  available,
  onToggle,
}: {
  workspaceId: number;
  kinds: AppKind[];
  /** This workspace's stored choices, by kind. */
  preferences: AppPreference[];
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
              className="flex min-h-[var(--row-height)] flex-wrap items-center gap-[var(--space-3)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0"
            >
              <span
                aria-hidden="true"
                className={included ? 'text-ember-bright' : 'text-ink-1'}
              >
                {included ? '●' : '○'}
              </span>

              <span className="t-ui shrink-0 text-ink-1">{appKindLabel(report.kind)}</span>

              <span className="t-value ml-auto min-w-0 truncate text-ink-0">
                {report.presence.state === 'available' ? (
                  report.presence.name
                ) : (
                  <span className="text-ink-1">Not installed</span>
                )}
              </span>

              <AppChooser
                workspaceId={workspaceId}
                kind={report.kind}
                preferred={
                  preferences.find((preference) => preference.kind === report.kind)
                    ?.application ?? null
                }
              />

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
