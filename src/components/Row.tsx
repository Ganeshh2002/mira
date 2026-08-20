import type { ReactNode } from 'react';

/**
 * The workhorse: mark · label · value. Ports, files, commits, shelf items and
 * capabilities are all Rows (design-system §8).
 */
export function Row({
  mark,
  label,
  value,
  detail,
}: {
  // `| undefined` is explicit because exactOptionalPropertyTypes is on: a caller
  // that computes an optional value passes it directly rather than spreading a
  // conditional object.
  mark?: ReactNode | undefined;
  label: string;
  value?: ReactNode | undefined;
  detail?: string | undefined;
}) {
  return (
    <div className="flex flex-col gap-[var(--space-1)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0">
      <div className="flex min-h-[var(--row-height)] items-center gap-[var(--space-3)]">
        {mark ? <span className="w-[1em] shrink-0 text-center">{mark}</span> : null}
        <span className="t-ui shrink-0 text-ink-1">{label}</span>
        {value ? (
          <span className="t-value ml-auto min-w-0 truncate text-ink-0">{value}</span>
        ) : null}
      </div>
      {detail ? <p className="t-ui m-0 text-ink-2">{detail}</p> : null}
    </div>
  );
}
