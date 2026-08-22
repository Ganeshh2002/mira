import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useId, useRef, useState } from 'react';

import type { ActionOffer } from '../bindings/ActionOffer';
import type { WorkspaceAction } from '../bindings/WorkspaceAction';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';
import { Button } from './Button';
import { Icon, type IconName } from './Icon';

/**
 * The actions that belong to this workspace.
 *
 * **Not a command palette, and not a way to run anything.** Every row is one
 * entry from a catalogue of six compiled into Mira, each naming something the
 * app could already do — open the editor, open a terminal, show the folder, open
 * the running service, mark the workspace opened, read everything again. There is
 * no field to type into, because there is nothing to type: an action is chosen
 * from a menu Mira produced and sent back as the identity Mira gave it
 * ([ADR-0021](../../docs/adr/0021-workspace-actions.md)).
 *
 * **Every row says what it will do** before it is pressed, in a sentence
 * underneath. That is the whole safety story made visible: you can read an
 * action, so a list of them is not a list of surprises.
 *
 * An action that cannot be done right now is a **sentence rather than a greyed-out
 * button** (`information-architecture.md` §5). That includes the ambiguous case —
 * two of this workspace's services running, and no way to know which one "open the
 * running service" meant. Mira says so and points at the Services list instead of
 * guessing, because guessing would open something nobody chose.
 *
 * Nothing here stops anything. There is no Stop, no Kill, no Restart, and no
 * disabled control hinting that one is coming.
 */
export function ActionsPanel({ workspaceId }: { workspaceId: number }) {
  const client = useQueryClient();

  const actions = useQuery({
    queryKey: workspaceKeys.actions(workspaceId),
    queryFn: () => commands.workspacesActions(workspaceId),
  });

  const catalogue = useQuery({
    queryKey: workspaceKeys.actionCatalogue(workspaceId),
    queryFn: () => commands.workspacesActionCatalogue(workspaceId),
  });

  function refresh() {
    void client.invalidateQueries({ queryKey: workspaceKeys.actions(workspaceId) });
    void client.invalidateQueries({ queryKey: workspaceKeys.actionCatalogue(workspaceId) });
  }

  const set = useMutation({
    mutationFn: ({ action, wanted }: { action: string; wanted: boolean }) =>
      commands.workspacesSetAction(workspaceId, action, wanted),
    onSuccess: refresh,
  });

  const chosen = actions.data ?? [];
  const failure = actions.error ?? set.error;

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <div className="flex items-baseline justify-between gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">Actions</h2>
        <AddAction
          offers={catalogue.data ?? []}
          pending={catalogue.isPending}
          onAdd={(action) => set.mutate({ action, wanted: true })}
        />
      </div>

      {chosen.length === 0 ? (
        <p className="t-ui m-0 text-ink-1">
          {actions.isPending
            ? 'Reading what this workspace does…'
            : 'No actions yet. Add the few this workspace actually uses — Mira will not add any on its own.'}
        </p>
      ) : (
        <ul
          aria-label="Workspace actions"
          className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
        >
          {chosen.map((action) => (
            <ActionRow
              key={action.id}
              action={action}
              workspaceId={workspaceId}
              onRemove={() => set.mutate({ action: action.id, wanted: false })}
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

/** One action: what it is, what it will do, and whether it can be done now. */
function ActionRow({
  action,
  workspaceId,
  onRemove,
}: {
  action: WorkspaceAction;
  workspaceId: number;
  onRemove: () => void;
}) {
  const perform = useMutation({
    mutationFn: () => commands.workspacesPerformAction(workspaceId, action.id),
  });

  const unknown = action.state.kind === 'unknown';
  const ready = action.state.kind === 'ready';
  const name = unknown ? action.id : action.label;

  return (
    <li className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0">
      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        <Icon name={icon(action.icon)} className={ready ? 'text-ink-0' : 'text-ink-2'} />
        <span className={`t-ui ${unknown ? 'text-ink-2' : 'text-ink-0'}`}>{name}</span>
        {ready && action.state.kind === 'ready' && action.state.detail ? (
          <span className="t-micro text-ink-1">{action.state.detail}</span>
        ) : null}

        <span className="ml-auto flex shrink-0 gap-[var(--space-2)]">
          {ready ? (
            <Button onClick={() => perform.mutate()} aria-label={`${action.label}`}>
              Do it
            </Button>
          ) : null}
          <Button onClick={onRemove} aria-label={`Remove ${name} from this workspace`}>
            <span className="flex items-center gap-[var(--space-1)]">
              <Icon name="remove" />
              Remove
            </span>
          </Button>
        </span>
      </div>

      <p className="t-ui m-0 text-ink-1">{explain(action)}</p>

      {perform.data ? (
        <p role="status" className="t-ui m-0 text-signal-ok">
          {perform.data.happened}
        </p>
      ) : null}
      {perform.error ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(perform.error)}
        </p>
      ) : null}
    </li>
  );
}

/**
 * The sentence under the row.
 *
 * Ready actions say what they will do; unavailable ones say why they cannot; an
 * identity Mira no longer has says exactly that, and is never quietly matched to
 * the nearest thing.
 */
function explain(action: WorkspaceAction): string {
  switch (action.state.kind) {
    case 'ready':
      return action.describes;
    case 'unavailable':
      return action.state.reason;
    case 'unknown':
      return 'This version of Mira has no action by that name. Removing it is safe — nothing was ever going to happen.';
  }
}

/** Catalogue icons are names from the same set; anything unexpected is neutral. */
function icon(name: string): IconName {
  const known: IconName[] = ['editor', 'terminal', 'service', 'folder', 'clock', 'refresh'];
  return known.find((one) => one === name) ?? 'application';
}

/**
 * Add one of Mira's actions to this workspace.
 *
 * A menu of the catalogue, never a field. Each row carries the same sentence the
 * action will carry once it is added, so the choice is made with the same
 * information the button will show.
 */
function AddAction({
  offers,
  pending,
  onAdd,
}: {
  offers: ActionOffer[];
  pending: boolean;
  onAdd: (action: string) => void;
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

  const available = offers.filter((offer) => !offer.chosen);

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
        Add an action
        <span aria-hidden="true">▾</span>
      </button>

      {open ? (
        <div
          id={id}
          role="menu"
          aria-label="Add an action"
          className="absolute right-0 top-[calc(100%+var(--space-1))] z-10 flex max-h-[var(--menu-height)] w-[var(--menu-width)] max-w-[80vw] flex-col overflow-y-auto rounded-sm border border-line bg-ground-1 py-[var(--space-1)]"
        >
          {pending ? (
            <p className="t-ui m-0 px-[var(--space-3)] py-[var(--space-1)] text-ink-2">
              Reading the catalogue…
            </p>
          ) : available.length === 0 ? (
            <p className="t-ui m-0 px-[var(--space-3)] py-[var(--space-1)] text-ink-2">
              This workspace already has every action Mira has.
            </p>
          ) : (
            available.map((offer) => (
              <button
                key={offer.id}
                type="button"
                role="menuitem"
                onClick={() => {
                  onAdd(offer.id);
                  setOpen(false);
                  trigger.current?.focus();
                }}
                className="t-ui flex cursor-default flex-col gap-[var(--space-1)] px-[var(--space-3)] py-[var(--space-2)] text-left text-ink-0 hover:bg-ground-2"
              >
                <span className="flex items-center gap-[var(--space-2)]">
                  <Icon name={icon(offer.icon)} />
                  {offer.label}
                </span>
                <span className="t-micro text-ink-2">{offer.describes}</span>
              </button>
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}
