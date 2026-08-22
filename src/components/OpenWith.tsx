import { useMutation, useQuery } from '@tanstack/react-query';

import type { AppId } from '../bindings/AppId';
import type { AppKind } from '../bindings/AppKind';
import type { AppPreference } from '../bindings/AppPreference';
import type { AppReport } from '../bindings/AppReport';
import { appKindLabel } from '../lib/applications';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';
import { Button } from './Button';

/**
 * Opening this workspace's project in a real application.
 *
 * Two buttons at most, and each one sends **a workspace id and a kind**. There
 * is no path here, no program name and no command: which directory is resolved
 * in Rust from the project row, and which application from a table compiled into
 * the binary. That is the whole reason the action is shaped this way rather than
 * as "open `/home/dev/aviora` in `code`" (`security-and-privacy.md` §5).
 *
 * **The button says what will actually open.** If the workspace chose one, that
 * is the name on it; otherwise it is whatever Mira would pick. A button reading
 * "Editor · " plus a name the workspace did not choose would be a lie
 * about what pressing it does ([ADR-0019](../../docs/adr/0019-application-preferences.md)).
 *
 * A kind with nothing behind it is stated rather than disabled. A greyed-out
 * button says "later"; a machine with no editor is not waiting for anything, and
 * the sentence is the honest version (`information-architecture.md` §5).
 *
 * Nothing here can stop what it started. A launched application belongs to the
 * desktop, not to Mira.
 */
export function OpenWith({
  workspaceId,
  openable,
  preferences,
}: {
  workspaceId: number;
  openable: AppReport[];
  /** This workspace's choices, so a button names what it will actually open. */
  preferences: AppPreference[];
}) {
  const launch = useMutation({
    mutationFn: (kind: AppKind) => commands.workspacesLaunch(workspaceId, kind),
  });

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <h2 className="t-label text-ink-1">Open with</h2>

      <div
        role="group"
        aria-label="Open with"
        className="flex flex-wrap items-center gap-[var(--space-2)]"
      >
        {openable.map((report) =>
          report.presence.state === 'available' ? (
            <OpenButton
              key={report.kind}
              workspaceId={workspaceId}
              kind={report.kind}
              preferred={
                preferences.find((preference) => preference.kind === report.kind)
                  ?.application ?? null
              }
              found={report.presence.name}
              disabled={launch.isPending}
              onOpen={() => launch.mutate(report.kind)}
            />
          ) : (
            <span key={report.kind} className="t-ui text-ink-1">
              {appKindLabel(report.kind)} · nothing here Mira can open
            </span>
          ),
        )}
      </div>

      {launch.isSuccess ? (
        <p className="t-ui m-0 text-ink-1">
          {launch.data.application === null ? 'Opened.' : `Opened ${launch.data.application}.`}
        </p>
      ) : null}

      {launch.error ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(launch.error)}
        </p>
      ) : null}
    </section>
  );
}

/**
 * One kind's button, named after what will actually open.
 *
 * A chosen application that is no longer here does not get a button: pressing it
 * could only fail, and offering it would suggest Mira might quietly open
 * something else. The row says which one is gone and the chooser in Context is
 * where another is picked.
 */
function OpenButton({
  workspaceId,
  kind,
  preferred,
  found,
  disabled,
  onOpen,
}: {
  workspaceId: number;
  kind: AppKind;
  preferred: AppId | null;
  /** What Mira found for this kind, used until the choice has resolved. */
  found: string;
  disabled: boolean;
  onOpen: () => void;
}) {
  const chosen = useQuery({
    queryKey: [...workspaceKeys.chosen(workspaceId), kind, preferred],
    queryFn: () => commands.workspacesChosen(workspaceId, kind),
  });

  const label = appKindLabel(kind);
  const resolved = chosen.data;

  if (resolved?.state === 'missing' || resolved?.state === 'notOpenable') {
    return (
      <span className="t-ui text-signal-warn">
        {label} · {resolved.name} is not available here
      </span>
    );
  }
  if (resolved?.state === 'unknown') {
    return (
      <span className="t-ui text-signal-warn">
        {label} · {resolved.id} is not something Mira looks for here
      </span>
    );
  }

  const naming =
    resolved?.state === 'ready'
      ? resolved.name
      : ((resolved?.state === 'automatic' ? resolved.application : null) ?? found);

  return (
    <Button onClick={onOpen} disabled={disabled}>
      {label} · {naming}
    </Button>
  );
}
