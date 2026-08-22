import { useMutation } from '@tanstack/react-query';

import type { PackageBoundary } from '../bindings/PackageBoundary';
import type { Service } from '../bindings/Service';
import type { ServiceObservation } from '../bindings/ServiceObservation';
import type { Unattributed } from '../bindings/Unattributed';
import { commands, describeUnknown } from '../lib/ipc';
import { Button } from './Button';
import { Freshness } from './Freshness';

/**
 * What is listening, grouped by the package it runs from.
 *
 * Every action here is read-only: open the address, or copy something. Stopping
 * a service is a later slice with its own confirmation and refusal design, and
 * its absence is deliberate rather than pending — there is no disabled Stop
 * button hinting at one (`information-architecture.md` §5).
 */
export function ServicesPanel({
  observation,
  services,
  unplaced,
}: {
  observation: ServiceObservation;
  services: Service[];
  unplaced: Service[];
}) {
  if (observation.error) {
    return (
      <section className="flex flex-col gap-[var(--space-2)]">
        <h2 className="t-label text-ink-1">Services</h2>
        <div className="rounded-md border border-line bg-ground-1 px-[var(--space-3)] py-[var(--space-2)]">
          <p className="t-body m-0 text-signal-warn">Port information unavailable</p>
          <p className="t-ui m-0 text-ink-1">{observation.error}</p>
        </div>
      </section>
    );
  }

  const shown = [...services, ...unplaced];
  if (shown.length === 0) return null;

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <div className="flex items-baseline justify-between gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">Services</h2>
        <Freshness observedAt={observation.observedAt} />
      </div>

      <ul
        aria-label="Services"
        className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
      >
        {group(services).map(([label, inGroup]) => (
          <li key={label} className="border-b border-line last:border-b-0">
            {label ? (
              <p className="t-micro m-0 px-[var(--space-3)] pt-[var(--space-2)] text-ink-2">
                {label}
              </p>
            ) : null}
            <ul className="m-0 flex list-none flex-col p-0">
              {inGroup.map((service) => (
                <ServiceRow
                  key={rowKey(service)}
                  service={service}
                  at={observation.services.indexOf(service)}
                />
              ))}
            </ul>
          </li>
        ))}

        {unplaced.length > 0 ? (
          <li className="border-b border-line last:border-b-0">
            <p className="t-micro m-0 px-[var(--space-3)] pt-[var(--space-2)] text-ink-2">
              Unattributed
            </p>
            <ul className="m-0 flex list-none flex-col p-0">
              {unplaced.map((service) => (
                <ServiceRow
                  key={rowKey(service)}
                  service={service}
                  at={observation.services.indexOf(service)}
                />
              ))}
            </ul>
          </li>
        ) : null}
      </ul>
    </section>
  );
}

/** Services by the package they run from; the project itself sorts first. */
function group(services: Service[]): [string, Service[]][] {
  const groups = new Map<string, Service[]>();

  for (const service of services) {
    const boundary =
      service.attribution.kind === 'project' ? service.attribution.package : null;
    const label = boundary ? boundary.path : '';
    groups.set(label, [...(groups.get(label) ?? []), service]);
  }

  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
}

function rowKey(service: Service): string {
  return `${service.listener.port}:${service.listener.pid ?? 'none'}`;
}

/**
 * One observed service.
 *
 * `at` is this row's position in the snapshot's own list, and it is what Open
 * sends. The port beside it is a **reading being displayed**; the position is
 * the only thing that travels back, so there is no number of the caller's
 * choosing on the wire (ADR-0020).
 */
function ServiceRow({ service, at }: { service: Service; at: number }) {
  const open = useMutation({
    mutationFn: () => commands.openService(at),
  });
  const copy = useMutation({ mutationFn: (text: string) => write(text) });

  const url = `http://localhost:${service.listener.port}`;
  const failure = open.error ?? copy.error;

  return (
    <li className="flex flex-col gap-[var(--space-1)] px-[var(--space-3)] py-[var(--space-2)]">
      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        <span aria-hidden="true" className="text-ember-bright">
          ●
        </span>
        <span className="t-value text-ink-0">:{service.listener.port}</span>
        <span className="t-ui min-w-0 truncate text-ink-1">
          {service.process?.name ?? 'unknown process'}
          {service.listener.pid === null ? '' : ` · PID ${service.listener.pid}`}
        </span>
        <span className="t-ui text-ink-1">{service.listener.localAddress}</span>

        <span className="ml-auto flex shrink-0 gap-[var(--space-2)]">
          <Button onClick={() => open.mutate()}>Open</Button>
          <Button onClick={() => copy.mutate(url)}>Copy URL</Button>
          <Button onClick={() => copy.mutate(String(service.listener.port))}>Copy port</Button>
          {service.listener.pid === null ? null : (
            <Button onClick={() => copy.mutate(String(service.listener.pid))}>Copy PID</Button>
          )}
        </span>
      </div>

      {service.attribution.kind === 'unattributed' ? (
        <p className="t-ui m-0 text-ink-1">{explain(service.attribution.reason)}</p>
      ) : null}

      {failure ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(failure)}
        </p>
      ) : null}
    </li>
  );
}

/**
 * Copy through the webview's own clipboard.
 *
 * No plugin and no new permission: the page writes to its own clipboard on a
 * user gesture, which is exactly the authority a web page already has. Adding a
 * clipboard plugin would widen the frontend's privilege surface to do something
 * the platform already allows.
 */
async function write(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

function explain(reason: Unattributed): string {
  switch (reason) {
    case 'noOwningProcess':
      return 'The operating system did not say which process is listening here.';
    case 'noWorkingDirectory':
      return 'This system does not let Mira see where that process is running from.';
    case 'outsideEveryProject':
      return 'It is running outside every project you have added.';
  }
}

/** Exported for the detail view's package grouping. */
export type { PackageBoundary };
