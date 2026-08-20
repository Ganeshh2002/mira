/**
 * The window ground.
 *
 * Mira does not draw a glass panel. The shell asks the operating system for its
 * standard window material — Liquid Glass on macOS 26, vibrancy before it, Mica on
 * Windows — and reports which treatment it actually achieved. This applies that
 * answer to the document so the stylesheet can let the material show through.
 *
 * The interface never asks which operating system it is on (ADR-0005, enforced by
 * a guard test). It asks what the ground is, and the platform layer decides.
 */
const TREATMENTS = ['systemMaterial', 'opaque'] as const;

export type SurfaceTreatment = (typeof TREATMENTS)[number];

export function applySurface(reported: string): void {
  const treatment: SurfaceTreatment = isTreatment(reported) ? reported : 'opaque';
  document.documentElement.dataset['surface'] = treatment;
}

function isTreatment(value: string): value is SurfaceTreatment {
  return (TREATMENTS as readonly string[]).includes(value);
}
