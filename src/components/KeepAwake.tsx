import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';

import type { KeepAwakeSpan } from '../bindings/KeepAwakeSpan';
import type { KeepAwakeState } from '../bindings/KeepAwakeState';
import { Icon } from './Icon';
import { Row } from './Row';
import { Section } from './Section';
import { commands, describeUnknown } from '../lib/ipc';
import { absoluteTime } from '../lib/time';

/** The backend says a span ended by itself. Nothing else changes without asking. */
const KEEP_AWAKE = 'mira://keep-awake';

const keepAwakeKey = ['keepAwake', 'state'] as const;

/**
 * The four choices, in the order they are offered.
 *
 * A closed vocabulary: the interface picks a word, never a number of minutes.
 * There is no "custom", and there is no way to ask for a week.
 */
const SPANS: { span: KeepAwakeSpan; label: string }[] = [
  { span: 'off', label: 'Off' },
  { span: 'thirtyMinutes', label: '30 minutes' },
  { span: 'oneHour', label: '1 hour' },
  { span: 'untilTurnedOff', label: 'Until turned off' },
];

/**
 * Keep Awake — asking the operating system not to fall asleep.
 *
 * A utility, sized like one. It lives in the tray menu, where a machine-wide
 * switch belongs, and here in Settings where its state and its reason can be read
 * (`design-system.md` §8; the tray is the quick surface, this is the explanation).
 *
 * What it does: holds one operating-system power request for as long as you said.
 * What it never does: simulate a keystroke, move a pointer, manufacture activity,
 * or hide anything from the tools that report it. Where a platform has no public
 * way for Mira to ask, this shows the reason rather than a control that does
 * nothing ([ADR-0014](../../docs/adr/0014-keep-awake.md)).
 */
export function KeepAwakePanel() {
  const client = useQueryClient();

  const state = useQuery({
    queryKey: keepAwakeKey,
    queryFn: commands.keepAwakeState,
  });

  // A span that reaches its end is the one change nobody pressed a button for.
  // The backend says so and the interface re-reads; there is no timer here and a
  // guard test fails the build if one appears.
  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;

    void listen(KEEP_AWAKE, () => {
      void client.invalidateQueries({ queryKey: keepAwakeKey });
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else stop = unlisten;
    });

    return () => {
      cancelled = true;
      stop?.();
    };
  }, [client]);

  const choose = useMutation({
    mutationFn: commands.keepAwakeSet,
    onSuccess: (next) => client.setQueryData(keepAwakeKey, next),
    onError: () => client.invalidateQueries({ queryKey: keepAwakeKey }),
  });

  if (state.isPending) {
    return (
      <Section label="Keep Awake">
        <Row label="Reading…" />
      </Section>
    );
  }

  if (state.isError) {
    return (
      <Section label="Keep Awake">
        <Row
          mark={<span className="text-signal-warn">◐</span>}
          label="Cannot be read"
          detail={describeUnknown(state.error)}
        />
      </Section>
    );
  }

  const current = state.data;

  if (current.state === 'unavailable') {
    return (
      <Section label="Keep Awake">
        <Row
          mark={<span className="text-ink-3">○</span>}
          label="Not available on this machine"
          detail={
            current.fallback
              ? `${current.reason}. Use ${current.fallback} instead.`
              : `${current.reason}.`
          }
        />
      </Section>
    );
  }

  const held = current.state === 'on' ? current.span : 'off';

  return (
    <Section label="Keep Awake">
      <Row
        mark={
          <span className={current.state === 'on' ? 'text-ember-bright' : 'text-ink-3'}>
            <Icon name="awake" />
          </span>
        }
        label={summary(current)}
        detail={until(current)}
      />

      <div
        role="radiogroup"
        aria-label="Keep Awake"
        className="flex flex-col gap-[var(--space-1)] px-[var(--space-3)] py-[var(--space-2)]"
      >
        {SPANS.map(({ span, label }) => (
          <button
            key={span}
            type="button"
            role="radio"
            aria-label={label}
            aria-checked={held === span}
            disabled={choose.isPending}
            onClick={() => choose.mutate(span)}
            className={`t-ui flex w-full cursor-default items-center gap-[var(--space-2)] rounded-sm px-[var(--space-2)] py-[var(--space-1)] text-left transition-colors duration-[var(--motion-instant)] ${
              held === span ? 'bg-ember-wash text-ink-0' : 'text-ink-1 hover:bg-ground-2'
            }`}
          >
            {/* The state is carried by a shape as well as by colour: a filled
                dot is on, a ring is off (`design-system.md` §5). */}
            <span aria-hidden="true" className="w-[1em] text-center">
              {held === span ? '●' : '○'}
            </span>
            {label}
          </button>
        ))}
      </div>

      {choose.isError ? (
        <Row
          mark={<span className="text-signal-danger">✕</span>}
          label="That did not take effect"
          detail={describeUnknown(choose.error)}
        />
      ) : null}

      <Row
        label="What this does"
        detail="Holds your machine's own power request, so it does not fall asleep. Mira never simulates typing or pointer movement, and never hides anything from the tools that report activity. It stops when you turn it off, when the time is up, and when Mira quits."
      />
    </Section>
  );
}

function summary(state: KeepAwakeState): string {
  if (state.state !== 'on') return 'Off — your machine sleeps as usual';
  switch (state.span) {
    case 'thirtyMinutes':
      return 'On · 30 minutes';
    case 'oneHour':
      return 'On · 1 hour';
    case 'untilTurnedOff':
      return 'On · until turned off';
    case 'off':
      return 'Off — your machine sleeps as usual';
  }
}

function until(state: KeepAwakeState): string | undefined {
  if (state.state !== 'on' || state.until === null) return undefined;
  return `Ends at ${absoluteTime(state.until)}.`;
}
