import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AppKind } from '../bindings/AppKind';
import type { Project } from '../bindings/Project';
import type { ProjectObservation } from '../bindings/ProjectObservation';
import type { Service } from '../bindings/Service';
import type { ServiceObservation } from '../bindings/ServiceObservation';
import type { Workspace } from '../bindings/Workspace';
import { ContextPanel } from '../components/ContextPanel';
import { Freshness } from '../components/Freshness';
import { GitPanel } from '../components/GitPanel';
import { MissingFolder } from '../components/MissingFolder';
import { OpenWith } from '../components/OpenWith';
import { Row } from '../components/Row';
import { Section } from '../components/Section';
import { ServicesPanel } from '../components/ServicesPanel';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';

/**
 * One workspace: the project underneath it, and everything Mira already knows
 * about that project.
 *
 * The actions follow the same rule as the readings: everything they act on
 * belongs to the project, so "Open with" opens the project's root, and a folder
 * that has gone missing removes the actions rather than failing them.
 *
 * Nothing here is a workspace's *own* runtime state, because a workspace has
 * none. Git, packages and services belong to the project, are observed once by
 * the scheduler, and are read by every workspace on that project — so two
 * workspaces never disagree, and switching between them costs no observation
 * (slice brief §12).
 */
export function WorkspaceDetail({
  workspace,
  project,
  observation,
  services,
  unplaced,
  serviceObservation,
}: {
  workspace: Workspace;
  project: Project;
  observation: ProjectObservation | undefined;
  services: Service[];
  unplaced: Service[];
  serviceObservation: ServiceObservation;
}) {
  const client = useQueryClient();

  const available = useQuery({
    queryKey: workspaceKeys.applications,
    queryFn: commands.workspacesApplications,
  });

  const openable = useQuery({
    queryKey: workspaceKeys.openable,
    queryFn: commands.workspacesOpenable,
  });

  const setApplications = useMutation({
    mutationFn: (kinds: AppKind[]) => commands.workspacesSetApplications(workspace.id, kinds),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: workspaceKeys.of(workspace.projectId) }),
  });

  function toggle(kind: AppKind, wanted: boolean) {
    const next = wanted
      ? [...workspace.applications, kind]
      : workspace.applications.filter((existing) => existing !== kind);
    setApplications.mutate(next);
  }

  return (
    <div className="flex min-w-0 flex-col gap-[var(--section-gap)]">
      <header className="flex flex-col gap-[var(--space-1)]">
        <h1 className="t-value-lg m-0 truncate text-ink-0">{workspace.name}</h1>
        {workspace.description ? (
          <p className="t-ui m-0 text-ink-1">{workspace.description}</p>
        ) : null}
      </header>

      <Section label="Project">
        <Row label={project.name} value={project.rootPath} />
      </Section>

      {observation && !observation.directoryExists ? (
        <MissingFolder path={project.rootPath} />
      ) : (
        <>
          {observation?.layout?.kind === 'monorepoRoot' &&
          observation.layout.packages.length > 0 ? (
            <section className="flex flex-col gap-[var(--space-2)]">
              <h2 className="t-label text-ink-1">Packages</h2>
              <ul
                aria-label="Packages"
                className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
              >
                {observation.layout.packages.map((pkg) => (
                  <li
                    key={pkg.path}
                    className="flex min-h-[var(--row-height)] items-center gap-[var(--space-3)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0"
                  >
                    <span className="t-ui min-w-0 shrink truncate text-ink-0">{pkg.name}</span>
                    <span className="t-value ml-auto min-w-0 truncate text-ink-1">
                      {pkg.path}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {observation?.git ? (
            <>
              <div className="flex items-baseline justify-between gap-[var(--space-3)]">
                <span className="t-label text-ink-1">Git</span>
                <Freshness observedAt={observation.observedAt} />
              </div>
              <GitPanel git={observation.git} labelled={false} />
            </>
          ) : null}

          <ServicesPanel
            observation={serviceObservation}
            services={services}
            unplaced={unplaced}
          />

          {openable.data ? (
            <OpenWith
              workspaceId={workspace.id}
              openable={openable.data}
              preferences={workspace.preferences}
            />
          ) : null}
        </>
      )}

      {available.isPending ? (
        <p className="t-ui text-ink-1">Looking for your applications…</p>
      ) : available.isError ? (
        <p role="alert" className="t-ui text-signal-danger">
          {describeUnknown(available.error)}
        </p>
      ) : (
        <ContextPanel
          workspaceId={workspace.id}
          kinds={workspace.applications}
          preferences={workspace.preferences}
          available={available.data}
          onToggle={toggle}
        />
      )}

      {setApplications.error ? (
        <p role="alert" className="t-ui m-0 text-signal-danger">
          {describeUnknown(setApplications.error)}
        </p>
      ) : null}

      <p className="t-ui m-0 text-ink-1">
        Opening a workspace shows you where things stand. Starting an editor or a terminal is
        the row above, and always something you asked for — Mira restores nothing on its own.
      </p>
    </div>
  );
}
