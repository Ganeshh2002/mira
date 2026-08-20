import { useQuery, useQueryClient } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';

import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import { commands } from './ipc';

/** The event the backend sends when an observation round finishes. */
const LIVE = 'mira://live';

export const liveKey = ['live', 'snapshot'] as const;

/**
 * What the observers last saw.
 *
 * The interface starts nothing. It asks once, then re-asks whenever the backend
 * says something moved — there is no `refetchInterval` here and a guard test
 * fails the build if one appears, because a timer in the webview would poll
 * straight through the scheduler's gate and past the idle-CPU budget.
 *
 * The first read is a `refresh` rather than a `snapshot`: the scheduler waits out
 * its first interval before doing anything, so asking for the snapshot on mount
 * would open the window onto five seconds of nothing.
 */
export function useLive() {
  const client = useQueryClient();

  const snapshot = useQuery({
    queryKey: liveKey,
    queryFn: commands.liveRefresh,
  });

  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;

    void listen(LIVE, () => {
      void client.invalidateQueries({ queryKey: liveKey });
    }).then((unlisten) => {
      // The view may have gone before `listen` resolved; unsubscribe at once
      // rather than leaking a listener that outlives what it updates.
      if (cancelled) unlisten();
      else stop = unlisten;
    });

    return () => {
      cancelled = true;
      stop?.();
    };
  }, [client]);

  return snapshot;
}

/** What was observed of one project, if anything has been yet. */
export function observationOf(live: LiveSnapshot | undefined, projectId: number) {
  return live?.projects.find((observation) => observation.projectId === projectId);
}

/** The services placed in one project. */
export function servicesOf(live: LiveSnapshot | undefined, projectId: number) {
  return (live?.services.services ?? []).filter(
    (service) =>
      service.attribution.kind === 'project' && service.attribution.projectId === projectId,
  );
}

/**
 * The services Mira could not place, and which might still be this project's.
 *
 * A project's view answers "what is running in *this* project", so a listener
 * Mira positively determined is outside every project — another application's
 * helper, an editor's language server — is noise there and is left out. Those
 * belong in the machine-wide Ports view.
 *
 * The exception is the one that matters on Windows: where the platform will not
 * expose a process's working directory, or will not say which process owns a
 * socket, Mira did not decide the service is elsewhere — it failed to tell.
 * Hiding those would leave a Windows user staring at an empty list while their
 * dev server is plainly running, so they are shown, with the reason.
 */
export function unplacedServices(live: LiveSnapshot | undefined) {
  return (live?.services.services ?? []).filter(
    (service) =>
      service.attribution.kind === 'unattributed' &&
      service.attribution.reason !== 'outsideEveryProject',
  );
}
