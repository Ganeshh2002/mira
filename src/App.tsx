import { useQuery } from '@tanstack/react-query';

import { commands, MiraCommandError } from './lib/ipc';
import { Foundation } from './surfaces/Foundation';
import { Settings } from './surfaces/Settings';

/** Mira has five surfaces. Slice 0 ships two: the main window and Settings. */
export type Surface = 'main' | 'settings';

export function App({ surface }: { surface: Surface }) {
  const query = useQuery({
    queryKey: ['app', 'foundationStatus'],
    queryFn: commands.foundationStatus,
  });

  return (
    <main className="mx-auto flex h-full max-w-[720px] flex-col gap-[var(--section-gap)] p-[var(--space-6)]">
      <header className="flex flex-col gap-[var(--space-1)]">
        <h1 className="t-value-lg m-0 text-ink-0">Mira</h1>
        <p className="t-ui m-0 text-ink-2">Foundation build</p>
      </header>

      {query.isPending ? (
        <p className="t-ui text-ink-2">Reading the foundation…</p>
      ) : query.isError ? (
        <p className="t-body text-signal-danger">{describe(query.error)}</p>
      ) : surface === 'settings' ? (
        <Settings status={query.data} />
      ) : (
        <Foundation status={query.data} />
      )}
    </main>
  );
}

function describe(error: unknown): string {
  return error instanceof MiraCommandError ? error.message : String(error);
}
