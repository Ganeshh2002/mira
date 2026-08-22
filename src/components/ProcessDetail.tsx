import type { ProcessFacts } from '../bindings/ProcessFacts';
import { Icon } from './Icon';

/**
 * What Mira knows about the process behind a listener, inline.
 *
 * Inline rather than in a modal, deliberately. This is a *reading*, and a
 * reading you have to open a window to see is one you stop checking. It sits
 * under the row it belongs to and adds one line.
 *
 * **There is no command line here and there will not be.** A process's argv
 * routinely carries credentials — `--password=`, `PGPASSWORD=`, a token inside a
 * `DATABASE_URL`, an API key a task runner passed down. Putting that on screen
 * puts it in every screenshot and every shoulder-glance. Redaction was
 * considered and rejected: it is a blocklist, and blocklists leak
 * ([ADR-0022](../../docs/adr/0022-process-detail.md)).
 *
 * Every field is optional because every one of them is a fact some platform
 * declines to give, and an absent fact is shown as absent rather than as zero.
 * CPU is absent on the first reading too: a share is a *rate*, and Mira has not
 * measured one until it has two samples — saying "0%" there would call a process
 * idle that might be burning a core.
 */
export function ProcessDetail({ process }: { process: ProcessFacts | null }) {
  if (!process) {
    return (
      <p className="t-ui m-0 text-ink-2">
        The operating system did not say which process is listening here.
      </p>
    );
  }

  return (
    <dl className="t-ui m-0 flex flex-wrap items-baseline gap-x-[var(--space-4)] gap-y-[var(--space-1)] text-ink-1">
      <Fact icon="cpu" label="CPU" value={share(process.cpuShare)} />
      <Fact icon="memory" label="Memory" value={size(process.memoryBytes)} />
      <Fact icon="clock" label="Running for" value={duration(process.uptimeSeconds)} />
    </dl>
  );
}

/**
 * One labelled reading.
 *
 * The icon is a companion to the word, never a replacement for it: the row reads
 * correctly with every icon stripped out, which is the rule the icon set was
 * built under (`design-system.md` §8).
 */
function Fact({
  icon,
  label,
  value,
}: {
  icon: 'cpu' | 'memory' | 'clock';
  label: string;
  value: string | null;
}) {
  return (
    <div className="flex items-baseline gap-[var(--space-1)]">
      <Icon name={icon} className="self-center text-ink-2" />
      <dt className="t-micro text-ink-2">{label}</dt>
      <dd className={`m-0 ${value ? 'text-ink-0' : 'text-ink-2'}`}>
        {value ?? 'not measured yet'}
      </dd>
    </div>
  );
}

/** A share of one core. Over 100 is real on a multi-core machine, and is shown. */
function share(value: number | null): string | null {
  if (value === null) return null;
  return `${value.toFixed(value < 10 ? 1 : 0)}%`;
}

/** Bytes, in the unit a person would say them in. */
function size(bytes: number | null): string | null {
  if (bytes === null) return null;

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/**
 * How long it has been up, coarsely.
 *
 * Coarse on purpose: "3h" is the useful answer and "3h 14m 22s" is a number that
 * changes while you read it.
 */
function duration(seconds: number | null): string | null {
  if (seconds === null) return null;
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86_400) {
    const hours = Math.floor(seconds / 3_600);
    const minutes = Math.floor((seconds % 3_600) / 60);
    return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`;
  }
  return `${Math.floor(seconds / 86_400)}d`;
}
