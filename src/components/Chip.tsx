import type { ReactNode } from 'react';

/** Micro mono text on a raised ground. Counts, states, short facts. */
export function Chip({ children }: { children: ReactNode }) {
  return (
    <span className="t-micro rounded-sm bg-ground-3 px-[var(--space-2)] py-[var(--space-1)] text-ink-1">
      {children}
    </span>
  );
}
