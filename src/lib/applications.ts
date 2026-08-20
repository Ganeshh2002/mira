import type { AppKind } from '../bindings/AppKind';

/**
 * The word for a kind of application.
 *
 * Interface copy, kept out of the backend and out of the wire: Rust sends the
 * variant, and the sentence is written here. Nothing in this file names a
 * specific application — every application name on screen arrived from
 * discovery, which is what stops the interface claiming something the machine
 * was never asked about (`security-and-privacy.md` §5, slice brief §6).
 */
export function appKindLabel(kind: AppKind): string {
  switch (kind) {
    case 'editor':
      return 'Editor';
    case 'terminal':
      return 'Terminal';
    case 'browser':
      return 'Browser';
  }
}
