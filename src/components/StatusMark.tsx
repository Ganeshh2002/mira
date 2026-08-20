import type { CapabilityStatus } from '../bindings/CapabilityStatus';

/**
 * The status language, in one place (design-system §5).
 *
 * Colour is never the only channel: each state carries a distinct glyph and an
 * accessible name, so the mark survives every form of colour blindness, every
 * monitor, and a screen reader.
 */
const MARKS = {
  full: { glyph: '●', name: 'Full', className: 'text-signal-ok' },
  degraded: { glyph: '◐', name: 'Degraded', className: 'text-signal-warn' },
  unavailable: { glyph: '⌀', name: 'Unavailable', className: 'text-ink-3' },
} as const;

export function StatusMark({ status }: { status: CapabilityStatus }) {
  const mark = MARKS[status.state];

  return (
    <span
      role="img"
      aria-label={mark.name}
      title={mark.name}
      className={`select-none ${mark.className}`}
    >
      {mark.glyph}
    </span>
  );
}

/** The word for a status, for places that show text rather than a mark. */
export function statusName(status: CapabilityStatus): string {
  return MARKS[status.state].name;
}
