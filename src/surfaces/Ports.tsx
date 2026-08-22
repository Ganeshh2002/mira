import { useMutation, useQuery } from '@tanstack/react-query';

import type { PortRow } from '../bindings/PortRow';
import { Freshness } from '../components/Freshness';
import { Icon } from '../components/Icon';
import { ProcessDetail } from '../components/ProcessDetail';
import { commands, describeUnknown } from '../lib/ipc';
import { portsKey } from '../lib/live';

/**
 * Every listening socket on this machine, grouped.
 *
 * The one place Mira shows machine-wide data prominently, because *"what has
 * :3000"* is asked without a project in mind (`information-architecture.md` §5).
 *
 * **This is not a workspace's Services list**, and the surface says so in its own
 * words rather than relying on the reader to notice. A workspace's Services
 * section answers "what did this workspace say matters" — curated, stored,
 * yours. This answers "what is listening on this computer" — observed, nobody's,
 * and gone the moment the process ends.
 *
 * Read-only throughout. Open is the only action, and it sends a **position** in
 * the list Mira produced rather than a port, so there is no request here through
 * which a page could reach a port Mira is not already watching. There is no Stop
 * and no Kill: termination is a later slice with its own confirmation and
 * refusal design, and its absence is deliberate rather than pending — no
 * greyed-out control hints at one.
 */
export function Ports() {
  const view = useQuery({ queryKey: portsKey, queryFn: commands.ports });

  if (view.isPending) {
    return <p className="t-ui text-ink-1">Reading what is listening…</p>;
  }
  if (view.isError) {
    return (
      <p role="alert" className="t-body text-signal-danger">
        {describeUnknown(view.error)}
      </p>
    );
  }

  const { projects, unattributed, observedAt, error } = view.data;
  const total = projects.reduce((n, group) => n + group.rows.length, 0) + unattributed.length;

  return (
    <div className="flex min-w-0 flex-col gap-[var(--section-gap)]">
      <header className="flex flex-col gap-[var(--space-1)]">
        <div className="flex items-baseline justify-between gap-[var(--space-3)]">
          <h1 className="t-value-lg m-0 flex items-center gap-[var(--space-2)] text-ink-0">
            <Icon name="ports" />
            Ports
          </h1>
          <Freshness observedAt={observedAt} />
        </div>
        <p className="t-ui m-0 text-ink-1">
          Everything listening on this computer — not just your projects, and not what a
          workspace watches. Mira reads this; it does not manage it.
        </p>
      </header>

      {error ? (
        <div className="rounded-md border border-line bg-ground-1 px-[var(--space-3)] py-[var(--space-2)]">
          <p className="t-body m-0 text-signal-warn">Port information unavailable</p>
          <p className="t-ui m-0 text-ink-1">{error}</p>
        </div>
      ) : total === 0 ? (
        <p className="t-ui m-0 text-ink-1">
          {observedAt === null
            ? 'Mira has not read the list of listening ports yet.'
            : 'Nothing is listening on this computer right now.'}
        </p>
      ) : (
        <>
          {projects.map((group) => (
            <Group key={group.projectId} label={group.project} rows={group.rows} />
          ))}

          {unattributed.length > 0 ? (
            <Group
              label="Not in any project"
              note="Mira could not place these, or they are running outside every project you have added."
              rows={unattributed}
            />
          ) : null}
        </>
      )}
    </div>
  );
}

/** One heading and its listeners. */
function Group({ label, note, rows }: { label: string; note?: string; rows: PortRow[] }) {
  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <h2 className="t-label text-ink-1">{label}</h2>
      {note ? <p className="t-ui m-0 text-ink-2">{note}</p> : null}
      <ul
        aria-label={label}
        className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
      >
        {rows.map((row) => (
          <Row key={`${row.port}:${row.process?.pid ?? 'none'}`} row={row} />
        ))}
      </ul>
    </section>
  );
}

/** One listener, with its process detail inline underneath. */
function Row({ row }: { row: PortRow }) {
  const open = useMutation({ mutationFn: () => commands.openService(row.at) });

  return (
    <li className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0">
      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        <span aria-hidden="true" className="text-ember-bright">
          ●
        </span>
        <span className="t-value text-ink-0">:{row.port}</span>
        <span className="t-ui min-w-0 truncate text-ink-1">
          {row.process?.name ?? 'unknown process'}
          {row.process ? ` · PID ${row.process.pid}` : ''}
        </span>
        <span className="t-ui text-ink-1">{row.address}</span>

        <button
          type="button"
          onClick={() => open.mutate()}
          aria-label={`Open port ${row.port} in the browser`}
          className="t-ui ml-auto shrink-0 cursor-default rounded-sm border border-line bg-ground-2 px-[var(--space-3)] py-[var(--space-1)] text-ink-0 transition-colors duration-[var(--motion-instant)] hover:bg-ground-3"
        >
          Open
        </button>
      </div>

      <ProcessDetail process={row.process} />

      {open.error ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(open.error)}
        </p>
      ) : null}
    </li>
  );
}
