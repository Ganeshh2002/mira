import { useEffect, useId, useRef, useState } from 'react';

import { Icon, type IconName } from './Icon';

/**
 * A `[ Label ▾ ]` control over a list Mira produced.
 *
 * Shared by the two places that need one — History's filters and a workspace's
 * application choices — because a second menu implementation is a second set of
 * keyboard behaviours to keep correct.
 */

export type MenuOption = {
  key: string;
  label: string;
  /** A short qualifier — a kind, a count. Never the only thing distinguishing two rows. */
  note: string | null;
  chosen: boolean;
  choose: () => void;
};

/**
 * One `[ Label ▾ ]` control.
 *
 * A button that opens a menu of things Mira offered. `Esc` closes it and gives
 * focus back, `↑`/`↓` move through it, and the chosen item says "chosen" in words
 * as well as showing a mark.
 */
export function Menu({
  icon,
  label,
  chosen,
  options,
  empty,
  footer,
  onClear,
}: {
  icon: IconName;
  label: string;
  chosen: string | null;
  options: MenuOption[];
  empty: string;
  footer: string | null;
  onClear: () => void;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();

  useEffect(() => {
    if (!open) return;

    function onDocument(event: MouseEvent) {
      if (!anchor.current?.contains(event.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', onDocument);
    return () => document.removeEventListener('mousedown', onDocument);
  }, [open]);

  function onKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      setOpen(false);
      trigger.current?.focus();
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;

    const items = Array.from(
      anchor.current?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]') ?? [],
    );
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const moving = at === -1 ? items[0] : items[event.key === 'ArrowDown' ? at + 1 : at - 1];
    if (!moving) return;

    event.preventDefault();
    moving.focus();
  }

  return (
    <div ref={anchor} onKeyDown={onKeyDown} className="relative">
      <button
        ref={trigger}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen(!open)}
        className={`t-ui flex cursor-default items-center gap-[var(--space-1)] rounded-sm border px-[var(--space-2)] py-[var(--space-1)] transition-colors duration-[var(--motion-instant)] ${
          chosen
            ? 'border-ember-dim bg-ember-wash text-ember-bright'
            : 'border-line bg-ground-2 text-ink-1 hover:bg-ground-3'
        }`}
      >
        <Icon name={icon} />
        {label}
        {chosen ? <span className="t-micro max-w-[12ch] truncate">: {chosen}</span> : null}
        <span aria-hidden="true">▾</span>
      </button>

      {open ? (
        <div
          id={id}
          role="menu"
          aria-label={label}
          className="absolute left-0 top-[calc(100%+var(--space-1))] z-10 flex max-h-[var(--menu-height)] w-[var(--menu-width)] max-w-[80vw] flex-col overflow-y-auto rounded-sm border border-line bg-ground-1 py-[var(--space-1)]"
        >
          {chosen ? (
            <button
              type="button"
              role="menuitem"
              onClick={() => {
                onClear();
                setOpen(false);
                trigger.current?.focus();
              }}
              className="t-ui cursor-default px-[var(--space-3)] py-[var(--space-1)] text-left text-ink-1 hover:bg-ground-2"
            >
              Any {label.toLowerCase()}
            </button>
          ) : null}

          {options.length === 0 ? (
            <p className="t-ui m-0 px-[var(--space-3)] py-[var(--space-1)] text-ink-2">
              {empty}
            </p>
          ) : (
            options.map((option) => (
              <button
                key={option.key}
                type="button"
                role="menuitem"
                onClick={() => {
                  option.choose();
                  setOpen(false);
                  trigger.current?.focus();
                }}
                className={`t-ui flex cursor-default items-baseline justify-between gap-[var(--space-2)] px-[var(--space-3)] py-[var(--space-1)] text-left hover:bg-ground-2 ${
                  option.chosen ? 'text-ember-bright' : 'text-ink-0'
                }`}
              >
                <span className="truncate">{option.label}</span>
                <span className="t-micro shrink-0 text-ink-2">
                  {option.chosen ? 'chosen' : option.note}
                </span>
              </button>
            ))
          )}

          {footer ? (
            <p className="t-micro m-0 border-t border-line px-[var(--space-3)] pt-[var(--space-1)] text-ink-2">
              {footer}
            </p>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
