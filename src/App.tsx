import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';

import { commands } from './lib/ipc';
import { applySurface } from './lib/surface';
import { Projects } from './surfaces/Projects';
import { SettingsSurface } from './surfaces/Settings';

/** Mira has five surfaces. Slice 1 ships two: the main window and Settings. */
export type Surface = 'main' | 'settings';

export function App({ surface }: { surface: Surface }) {
  useWindowGround();

  if (surface === 'settings') {
    return (
      <main className="mx-auto flex h-full max-w-[720px] flex-col gap-[var(--section-gap)] p-[var(--space-6)]">
        <SettingsSurface />
      </main>
    );
  }

  return (
    <main className="mx-auto flex h-full max-w-[1040px] flex-col p-[var(--space-6)]">
      <Projects />
    </main>
  );
}

/**
 * Ask the shell which ground it achieved, and apply it.
 *
 * Achieved, not intended: a machine that refused the material reports `opaque`,
 * and the window stays solid rather than translucent over nothing
 * (`platform-abstraction.md` §2, the parity rule).
 */
function useWindowGround() {
  const status = useQuery({
    queryKey: ['app', 'foundationStatus'],
    queryFn: commands.foundationStatus,
  });
  const reported = status.data?.surface;

  useEffect(() => {
    if (reported) applySurface(reported);
  }, [reported]);
}
