import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useId, useRef, useState } from 'react';

import type { ServiceOffer } from '../bindings/ServiceOffer';
import type { ServiceState } from '../bindings/ServiceState';
import type { WorkspaceService } from '../bindings/WorkspaceService';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';
import { Button } from './Button';
import { Icon } from './Icon';
import { ProcessDetail } from './ProcessDetail';

/**
 * The services this workspace watches.
 *
 * The narrowing ADR-0012 said was missing. A project's observations are shared —
 * one scan, read by every workspace on it — and this is where a workspace says
 * *which of them are the work*. Two workspaces on one repository can watch
 * different services and neither can see or change the other's list
 * ([ADR-0020](../../docs/adr/0020-workspace-services.md)).
 *
 * **Every row says what Mira actually knows**, which is why there are four
 * sentences and not two. "Not running" is a claim, and Mira may only make it
 * after looking; before the first scan, and after one that failed, the row says
 * so instead. A port something *else* has taken is its own state as well, because
 * reporting a stranger's process as your dev server would be Mira substituting
 * one thing for another — the failure `security-and-privacy.md` §6 forbids for
 * applications, applied here.
 *
 * Nothing on this surface can stop anything. Open is the only action, and it goes
 * to the same launcher an editor does, with an address Mira built in Rust from a
 * port it observed.
 */
export function WorkspaceServices({ workspaceId }: { workspaceId: number }) {
  const client = useQueryClient();

  const watched = useQuery({
    queryKey: workspaceKeys.services(workspaceId),
    queryFn: () => commands.workspacesServices(workspaceId),
  });

  const offers = useQuery({
    queryKey: workspaceKeys.offers(workspaceId),
    queryFn: () => commands.workspacesServiceOffers(workspaceId),
  });

  function refresh() {
    void client.invalidateQueries({ queryKey: workspaceKeys.services(workspaceId) });
    void client.invalidateQueries({ queryKey: workspaceKeys.offers(workspaceId) });
  }

  const watch = useMutation({
    mutationFn: (at: number) => commands.workspacesWatchService(workspaceId, at),
    onSuccess: refresh,
  });

  const forget = useMutation({
    mutationFn: (serviceId: number) => commands.workspacesForgetService(workspaceId, serviceId),
    onSuccess: refresh,
  });

  const services = watched.data ?? [];
  const failure = watched.error ?? watch.error ?? forget.error;

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <div className="flex items-baseline justify-between gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">Services</h2>
        <AddService
          offers={offers.data ?? []}
          pending={offers.isPending}
          onAdd={(at) => watch.mutate(at)}
        />
      </div>

      {services.length === 0 ? (
        <p className="t-ui m-0 text-ink-1">
          {watched.isPending
            ? 'Reading what this workspace watches…'
            : 'This workspace is not watching any services yet. Add one and it will be here every time you come back, running or not.'}
        </p>
      ) : (
        <ul
          aria-label="Watched services"
          className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
        >
          {services.map((service) => (
            <ServiceRow
              key={service.watched.id}
              service={service}
              workspaceId={workspaceId}
              onForget={() => forget.mutate(service.watched.id)}
            />
          ))}
        </ul>
      )}

      {failure ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(failure)}
        </p>
      ) : null}
    </section>
  );
}

/** One watched service: what it is, where it stands, and the two things you can do. */
function ServiceRow({
  service,
  workspaceId,
  onForget,
}: {
  service: WorkspaceService;
  workspaceId: number;
  onForget: () => void;
}) {
  const open = useMutation({
    mutationFn: () => commands.workspacesOpenService(workspaceId, service.watched.id),
  });

  const { tone, word } = describeState(service.state);
  const port = `:${service.watched.port}`;

  return (
    <li className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0">
      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        <Icon name="service" className={tone} />
        <span className="t-value text-ink-0">{port}</span>
        <span className={`t-ui ${tone}`}>{word}</span>
        {service.state.kind === 'running' ? (
          <span className="t-ui min-w-0 truncate text-ink-1">
            {service.state.process ?? 'unknown process'}
            {service.state.pid === null ? '' : ` · PID ${service.state.pid}`}
          </span>
        ) : null}

        <span className="ml-auto flex shrink-0 gap-[var(--space-2)]">
          {service.state.kind === 'running' ? (
            <Button onClick={() => open.mutate()}>Open</Button>
          ) : null}
          <Button onClick={onForget} aria-label={`Stop watching port ${service.watched.port}`}>
            <span className="flex items-center gap-[var(--space-1)]">
              <Icon name="remove" />
              Stop watching
            </span>
          </Button>
        </span>
      </div>

      <p className="t-ui m-0 text-ink-1">{explain(service.state, service.watched.port)}</p>

      {/*
        Only when it is actually running. A stopped service has no process, and
        a row of "not measured yet" under one would be three absences pretending
        to be information.
      */}
      {service.state.kind === 'running' ? (
        <ProcessDetail
          process={{
            pid: service.state.pid ?? 0,
            name: service.state.process ?? 'unknown process',
            executable: null,
            parent: null,
            workingDirectory: null,
            cpuShare: service.state.cpuShare,
            memoryBytes: service.state.memoryBytes,
            uptimeSeconds: service.state.uptimeSeconds,
          }}
        />
      ) : null}

      {open.error ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(open.error)}
        </p>
      ) : null}
    </li>
  );
}

/** The word on the row, and the colour that carries it. */
function describeState(state: ServiceState): { tone: string; word: string } {
  switch (state.kind) {
    case 'running':
      return { tone: 'text-signal-ok', word: 'Running' };
    case 'notRunning':
      return { tone: 'text-ink-2', word: 'Not running' };
    case 'taken':
      return { tone: 'text-signal-warn', word: 'Port taken' };
    case 'neverObserved':
      return { tone: 'text-ink-2', word: 'Never observed' };
    case 'unreadable':
      return { tone: 'text-signal-warn', word: 'Cannot tell' };
  }
}

/**
 * The sentence under the row.
 *
 * Four different sentences for four different truths. The two that matter most
 * are the last: neither of them says "not running", because Mira has not
 * established that and saying it would be telling somebody their server is down
 * when the truth is that Mira could not look (`information-architecture.md` §5).
 */
function explain(state: ServiceState, port: number): string {
  switch (state.kind) {
    case 'running':
      return `Listening on ${state.address}.`;
    case 'notRunning':
      return 'Nothing is listening here. Mira does not start it — that is still yours to do.';
    case 'taken':
      return state.process
        ? `Something else is on port ${port}: ${state.process}. Mira will not open it in place of yours.`
        : `Something else is on port ${port}, and Mira cannot tell what. It will not open it in place of yours.`;
    case 'neverObserved':
      return 'Mira has not read the list of listening ports yet.';
    case 'unreadable':
      return `Mira could not read the list of listening ports. ${state.reason}`;
  }
}

/**
 * Add one of the project's observed services to this workspace.
 *
 * A menu of what Mira found, never a field to type a port into. That is the whole
 * privilege the interface has here: pick a position in a list Mira produced. A
 * port somebody could type would be a port Mira never saw, and opening it would
 * be opening whatever happened to be on the number.
 */
function AddService({
  offers,
  pending,
  onAdd,
}: {
  offers: ServiceOffer[];
  pending: boolean;
  onAdd: (at: number) => void;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();

  useEffect(() => {
    if (!open) return;

    function onDocument(event: MouseEvent) {
      if (!anchor.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', onDocument);
    return () => document.removeEventListener('mousedown', onDocument);
  }, [open]);

  function onKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      setOpen(false);
      trigger.current?.focus();
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;

    const items = Array.from(
      anchor.current?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]') ?? [],
    );
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const moving = at === -1 ? items[0] : items[event.key === 'ArrowDown' ? at + 1 : at - 1];
    if (!moving) return;

    event.preventDefault();
    moving.focus();
  }

  const available = offers.filter((offer) => !offer.watched);

  return (
    <div ref={anchor} onKeyDown={onKeyDown} className="relative">
      <button
        ref={trigger}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen(!open)}
        className="t-ui flex cursor-default items-center gap-[var(--space-1)] rounded-sm border border-line bg-ground-2 px-[var(--space-2)] py-[var(--space-1)] text-ink-1 transition-colors duration-[var(--motion-instant)] hover:bg-ground-3"
      >
        <Icon name="add" />
        Add a service
        <span aria-hidden="true">▾</span>
      </button>

      {open ? (
        <div
          id={id}
          role="menu"
          aria-label="Add a service"
          className="absolute right-0 top-[calc(100%+var(--space-1))] z-10 flex max-h-[var(--menu-height)] w-[var(--menu-width)] max-w-[80vw] flex-col overflow-y-auto rounded-sm border border-line bg-ground-1 py-[var(--space-1)]"
        >
          {pending ? (
            <p className="t-ui m-0 px-[var(--space-3)] py-[var(--space-1)] text-ink-2">
              Looking at what this project is running…
            </p>
          ) : available.length === 0 ? (
            <p className="t-ui m-0 px-[var(--space-3)] py-[var(--space-1)] text-ink-2">
              {offers.length === 0
                ? 'Mira has not seen this project serving anything yet. Start it and it will appear here.'
                : 'Every service Mira has seen for this project is already on the list.'}
            </p>
          ) : (
            available.map((offer) => (
              <button
                key={offer.at}
                type="button"
                role="menuitem"
                onClick={() => {
                  onAdd(offer.at);
                  setOpen(false);
                  trigger.current?.focus();
                }}
                className="t-ui flex cursor-default items-baseline justify-between gap-[var(--space-2)] px-[var(--space-3)] py-[var(--space-1)] text-left text-ink-0 hover:bg-ground-2"
              >
                <span className="truncate">:{offer.port}</span>
                <span className="t-micro shrink-0 truncate text-ink-2">
                  {offer.process ?? offer.address}
                </span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
