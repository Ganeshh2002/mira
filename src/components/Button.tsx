import type { ButtonHTMLAttributes } from 'react';

/**
 * Three kinds only: quiet (default), accent (one per view maximum), danger
 * (terminate only). No icon-only buttons without an accessible name
 * (design-system §8).
 */
type Kind = 'quiet' | 'accent' | 'danger';

const KINDS: Record<Kind, string> = {
  quiet: 'border-line bg-ground-2 text-ink-0 hover:bg-ground-3',
  accent: 'border-ember-dim bg-ember-wash text-ember-bright hover:border-ember',
  danger: 'border-signal-danger bg-transparent text-signal-danger',
};

export function Button({
  kind = 'quiet',
  ...props
}: { kind?: Kind } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      {...props}
      className={`t-ui cursor-default rounded-sm border px-[var(--space-3)] py-[var(--space-1)] transition-colors duration-[var(--motion-instant)] ${KINDS[kind]}`}
    />
  );
}
