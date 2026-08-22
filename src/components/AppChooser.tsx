import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AppId } from '../bindings/AppId';
import type { AppKind } from '../bindings/AppKind';
import { Menu } from './Menu';
import { appKindLabel } from '../lib/applications';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';

/**
 * Which application this workspace uses for one kind.
 *
 * **The menu is a list Mira produced.** Every row comes back from
 * `workspaces.catalogue`, and choosing one sends back the identity it arrived
 * with — a catalogue id, never a path, a program name or a command. There is no
 * field here to type an application into, because there is no command that would
 * take one ([ADR-0019](../../docs/adr/0019-application-preferences.md)).
 *
 * **Automatic is the default and stays a real option.** It is the behaviour every
 * workspace had before anyone chose anything: the first entry in Mira's list that
 * this machine has. The menu says what that is today, so it is a description
 * rather than a mystery.
 *
 * **A choice that stopped being true says so.** An application uninstalled since
 * it was chosen is named on the row, with the menu right there to choose another.
 * Mira does not quietly open something else — that would be worse than the error,
 * because the person would never find out.
 */
export function AppChooser({
  workspaceId,
  kind,
  preferred,
}: {
  workspaceId: number;
  kind: AppKind;
  /** This workspace's stored choice, or `null` for automatic. */
  preferred: AppId | null;
}) {
  const client = useQueryClient();

  const catalogue = useQuery({
    queryKey: workspaceKeys.catalogue(kind),
    queryFn: () => commands.workspacesCatalogue(kind),
  });

  const chosen = useQuery({
    queryKey: [...workspaceKeys.chosen(workspaceId), kind, preferred],
    queryFn: () => commands.workspacesChosen(workspaceId, kind),
  });

  const prefer = useMutation({
    mutationFn: (application: AppId | null) =>
      commands.workspacesPrefer(workspaceId, kind, application),
    onSuccess: (updated) => {
      void client.invalidateQueries({ queryKey: workspaceKeys.of(updated.projectId) });
      void client.invalidateQueries({ queryKey: workspaceKeys.chosen(workspaceId) });
    },
  });

  const options = catalogue.data?.options ?? [];
  const automatic = catalogue.data?.automatic ?? null;
  const label = appKindLabel(kind);

  return (
    <div className="flex min-w-0 flex-col items-end gap-[var(--space-1)]">
      <Menu
        icon="application"
        label={`${label} application`}
        chosen={chosenLabel(chosen.data)}
        options={[
          {
            key: 'automatic',
            // What automatic does today is in the label, so the option is a
            // description rather than a mystery — including when the answer is
            // that this machine has nothing to pick.
            label: automatic ? `Automatic · ${automatic}` : 'Automatic · nothing here yet',
            note: null,
            chosen: preferred === null,
            choose: () => prefer.mutate(null),
          },
          ...options.map((option) => ({
            key: option.id,
            label: option.name,
            // Both facts in words, because a greyed row and a lit row are the
            // same row to somebody who cannot see the difference.
            note: !option.installed
              ? 'not installed'
              : option.openable
                ? 'installed'
                : 'cannot open a folder',
            chosen: preferred === option.id,
            // The id goes back exactly as it came. Nothing here builds one.
            choose: () => prefer.mutate(option.id),
          })),
        ]}
        empty={`Mira does not look for any ${label.toLowerCase()} on this platform.`}
        footer={
          options.length === 0
            ? `Mira does not look for any ${label.toLowerCase()} on this platform.`
            : null
        }
        onClear={() => prefer.mutate(null)}
      />

      {chosen.data ? <Standing chosen={chosen.data} /> : null}

      {prefer.error ? (
        <p role="alert" className="t-micro m-0 text-signal-danger">
          {describeUnknown(prefer.error)}
        </p>
      ) : null}
    </div>
  );
}

/** What the trigger shows: the choice, or nothing when it is on automatic. */
function chosenLabel(chosen: Chosen): string | null {
  if (!chosen) return null;

  switch (chosen.state) {
    case 'automatic':
      return null;
    case 'unknown':
      return chosen.id;
    default:
      return chosen.name;
  }
}

type Chosen = Awaited<ReturnType<typeof commands.workspacesChosen>> | undefined;

/**
 * The sentence under the menu, when there is one worth saying.
 *
 * Silent for the two states that are working as intended. A missing or
 * unopenable choice is the whole reason this component reports anything at all.
 */
function Standing({ chosen }: { chosen: NonNullable<Chosen> }) {
  const message = describeChosen(chosen);
  if (!message) return null;

  return (
    <p className="t-micro m-0 max-w-[var(--field-width)] text-right text-signal-warn">
      {message}
    </p>
  );
}

/** What to say about a resolved choice, or `null` when nothing needs saying. */
function describeChosen(chosen: NonNullable<Chosen>): string | null {
  switch (chosen.state) {
    case 'missing':
      return `${chosen.name} is not on this machine. Nothing else will be opened — choose another.`;
    case 'notOpenable':
      return `${chosen.name} is here, and is not something Mira can open a folder in.`;
    case 'unknown':
      return `This workspace uses ${chosen.id}, which Mira does not look for on this platform.`;
    default:
      return null;
  }
}
