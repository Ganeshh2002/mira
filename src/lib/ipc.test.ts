import { describe, expect, it } from 'vitest';

import type { MiraError } from '../bindings/MiraError';
import { describeError } from './ipc';

/**
 * Error copy is interface copy (`design-system.md` §9): plain, active, specific,
 * and written for the person reading it rather than the person who wrote the
 * `match` arm above it.
 */
describe('describing an error', () => {
  it('says what is wrong with an input without naming the field', () => {
    const error: MiraError = {
      kind: 'invalid',
      field: 'path',
      detail: '/home/dev/aviora is already open as "Aviora".',
    };

    // `field` is how the backend says which input failed. Rendering it produces
    // "path is not valid. …", which is a form-validation error from a program
    // that has no form. The detail is already the whole sentence.
    expect(describeError(error)).toBe('/home/dev/aviora is already open as "Aviora".');
  });

  it('names the thing that was missing', () => {
    expect(describeError({ kind: 'notFound', what: 'The folder for "Aviora"' })).toBe(
      'The folder for "Aviora" was not found.',
    );
  });

  it('gives a refusal the action that would fix it', () => {
    const said = describeError({
      kind: 'permissionDenied',
      what: '/home/dev/aviora',
      hint: 'Grant Mira access to this folder, or choose another one.',
    });

    expect(said).toContain('/home/dev/aviora');
    expect(said).toContain('Grant Mira access');
  });

  it('says how long it waited before giving up', () => {
    expect(
      describeError({ kind: 'timeout', operation: 'Reading Git for "Aviora"', afterMs: 2000 }),
    ).toBe('Reading Git for "Aviora" did not finish within 2000 ms.');
  });

  it('never leaves a message empty, whatever the variant', () => {
    const all: MiraError[] = [
      { kind: 'notFound', what: 'x' },
      { kind: 'permissionDenied', what: 'x', hint: 'y' },
      { kind: 'unsupported', capability: 'revealInFileManager', reason: 'y' },
      { kind: 'timeout', operation: 'x', afterMs: 1 },
      { kind: 'external', source: 'SQLite', detail: 'disk I/O error' },
      { kind: 'invalid', field: 'path', detail: 'No directory was chosen.' },
    ];

    for (const error of all) {
      expect(describeError(error).trim().length, error.kind).toBeGreaterThan(0);
    }
  });
});
