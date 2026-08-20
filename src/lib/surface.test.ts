import { beforeEach, describe, expect, it } from 'vitest';

import { applySurface } from './surface';

describe('the window ground', () => {
  beforeEach(() => {
    document.documentElement.removeAttribute('data-surface');
  });

  it('carries the treatment the platform actually achieved', () => {
    applySurface('systemMaterial');

    expect(document.documentElement.dataset['surface']).toBe('systemMaterial');
  });

  it('falls back to opaque where no material was applied', () => {
    applySurface('opaque');

    expect(document.documentElement.dataset['surface']).toBe('opaque');
  });

  it('treats an unknown treatment as opaque rather than guessing', () => {
    // The backend is the authority on this name. A value the interface does not
    // recognise must not leave the window transparent over nothing.
    applySurface('holographic');

    expect(document.documentElement.dataset['surface']).toBe('opaque');
  });
});
