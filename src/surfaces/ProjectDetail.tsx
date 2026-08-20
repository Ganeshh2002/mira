import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import type { Project } from '../bindings/Project';
import { Button } from '../components/Button';
import { Chip } from '../components/Chip';
import { GitPanel } from '../components/GitPanel';
import { LayoutPanel } from '../components/LayoutPanel';
import { Row } from '../components/Row';
import { MissingFolder } from '../components/MissingFolder';
import { Section } from '../components/Section';
import { ServicesPanel } from '../components/ServicesPanel';
import { commands, describeUnknown } from '../lib/ipc';
import { liveKey, observationOf, servicesOf, unplacedServices, useLive } from '../lib/live';
import { Freshness } from '../components/Freshness';

/**
 * One project: where it is, what Git says about it, and the way in.
 *
 * Context is read **on demand**, for the project on screen and no other. Nothing
 * polls and nothing watches — Slice 1 has no scheduler, and starting a timer
 * outside one is the thing `architecture.md` §6 exists to prevent.
 */
export function ProjectDetail({
  project,
  onRemoved,
}: {
  project: Project;
  onRemoved: () => void;
}) {
  const client = useQueryClient();
  const [confirming, setConfirming] = useState(false);

  const live = useLive();
  const observation = observationOf(live.data, project.id);

  const reveal = useMutation({ mutationFn: () => commands.projectsReveal(project.id) });
  const remove = useMutation({
    mutationFn: () => commands.projectsRemove(project.id),
    onSuccess: async () => {
      onRemoved();
      await client.invalidateQueries({ queryKey: ['projects'] });
      await client.invalidateQueries({ queryKey: liveKey });
    },
  });

  const failure = reveal.error ?? remove.error;

  return (
    <div className="flex min-w-0 flex-col gap-[var(--section-gap)]">
      <header className="flex flex-col gap-[var(--space-2)]">
        <h1 className="t-value-lg m-0 truncate text-ink-0">{project.name}</h1>
        <p className="t-ui m-0 truncate text-ink-1" title={project.rootPath}>
          {project.rootPath}
        </p>
        {project.markers.length > 0 ? (
          <div className="flex flex-wrap gap-[var(--space-2)]">
            {project.markers.map((marker) => (
              <Chip key={marker}>{marker}</Chip>
            ))}
          </div>
        ) : null}
      </header>

      {live.isPending && !observation ? (
        <p className="t-ui text-ink-1">Reading…</p>
      ) : live.isError ? (
        <Section label="Git">
          <Row
            mark={<span className="text-signal-warn">◐</span>}
            label="Could not be read"
            detail={describeUnknown(live.error)}
          />
        </Section>
      ) : !observation ? (
        <p className="t-ui text-ink-1">Not observed yet.</p>
      ) : !observation.directoryExists ? (
        <MissingFolder path={project.rootPath} />
      ) : (
        <>
          {observation.layout ? <LayoutPanel layout={observation.layout} /> : null}
          {observation.git ? (
            <>
              <div className="flex items-baseline justify-between gap-[var(--space-3)]">
                <span className="t-label text-ink-1">Git</span>
                <Freshness observedAt={observation.observedAt} />
              </div>
              <GitPanel git={observation.git} labelled={false} />
            </>
          ) : null}
          {observation.error ? (
            <p className="t-ui m-0 text-signal-warn">
              Could not be refreshed. {observation.error}
            </p>
          ) : null}
          <ServicesPanel
            observation={live.data?.services ?? { services: [], error: null, observedAt: null }}
            services={servicesOf(live.data, project.id)}
            unplaced={unplacedServices(live.data)}
          />
        </>
      )}

      {failure ? (
        <p role="alert" className="t-body m-0 text-signal-danger">
          {describeUnknown(failure)}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-[var(--space-3)]">
        <Button kind="accent" onClick={() => reveal.mutate()}>
          Open Project
        </Button>
        <Button onClick={() => void live.refetch()} disabled={live.isFetching}>
          {live.isFetching ? 'Refreshing…' : 'Refresh'}
        </Button>
        {confirming ? (
          <>
            <span className="t-ui text-ink-1">Remove “{project.name}” from Mira?</span>
            <Button kind="danger" onClick={() => remove.mutate()}>
              Remove
            </Button>
            <Button onClick={() => setConfirming(false)}>Cancel</Button>
          </>
        ) : (
          <Button onClick={() => setConfirming(true)}>Remove project</Button>
        )}
      </div>
      <p className="t-ui m-0 text-ink-1">
        Removing a project takes it out of Mira. The folder on disk is not touched.
      </p>
    </div>
  );
}
