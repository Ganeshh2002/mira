import { useMutation } from '@tanstack/react-query';

import type { AppKind } from '../bindings/AppKind';
import type { AppReport } from '../bindings/AppReport';
import { appKindLabel } from '../lib/applications';
import { commands, describeUnknown } from '../lib/ipc';
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
}: {
  workspaceId: number;
  openable: AppReport[];
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
            <Button
              key={report.kind}
              onClick={() => launch.mutate(report.kind)}
              disabled={launch.isPending}
            >
              {appKindLabel(report.kind)} · {report.presence.name}
            </Button>
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
