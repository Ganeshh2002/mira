import type { ReactNode } from 'react';

/**
 * Label plus content. A section is **absent when it has nothing to say** — never
 * rendered empty (design-system §8).
 */
export function Section({ label, children }: { label: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      {label ? <h2 className="t-label text-ink-1">{label}</h2> : null}
      <div className="overflow-hidden rounded-md border border-line bg-ground-1">
        {children}
      </div>
    </section>
  );
}
