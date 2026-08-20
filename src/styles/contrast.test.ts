import { describe, expect, it } from 'vitest';

// Read as text through Vite rather than through `node:fs`, so this test needs no
// Node types — and the frontend keeps having no way to reach a filesystem at all.
import app from './app.css?raw';
import projectDetail from '../surfaces/ProjectDetail.tsx?raw';
import projects from '../surfaces/Projects.tsx?raw';
import tokens from './tokens.css?raw';

/**
 * The contrast floors, measured rather than asserted.
 *
 * `design-system.md` §2 and §10 set AA as a floor for body text and UI, and the
 * window material makes that a question with a real answer: a translucent ground
 * over a light desktop is lighter than the token says, and the text on it is
 * correspondingly harder to read. This computes the worst case instead of
 * trusting it.
 */
const AA_BODY = 4.5;

/** A `--name: #rrggbb;` declaration from the first (Night) block that defines it. */
function token(name: string): string {
  const match = new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`).exec(tokens);
  if (!match?.[1]) throw new Error(`token --${name} is not defined`);
  return match[1];
}

/** The ground's opacity under the system material, read from the stylesheet. */
function groundOpacity(): number {
  const match = /--ground-0\)\s*(\d+)%,\s*transparent/.exec(app);
  if (!match?.[1]) throw new Error('the material ground is not defined in app.css');
  return Number(match[1]) / 100;
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
  return 0.2126 * channel(r!) + 0.7152 * channel(g!) + 0.0722 * channel(b!);
}

function contrast(a: string, b: string): number {
  const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (high! + 0.05) / (low! + 0.05);
}

/** `over` seen through `front` at `alpha`. */
function composite(front: string, over: string, alpha: number): string {
  const parts = [1, 3, 5].map((at) => {
    const f = Number.parseInt(front.slice(at, at + 2), 16);
    const b = Number.parseInt(over.slice(at, at + 2), 16);
    return Math.round(f * alpha + b * (1 - alpha));
  });
  return `#${parts.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
}

describe('text on the window ground', () => {
  it('clears AA on the solid ground', () => {
    const ground = token('ground-0');

    for (const ink of ['ink-0', 'ink-1', 'ink-2']) {
      expect(contrast(token(ink), ground), `${ink} on the solid ground`).toBeGreaterThanOrEqual(
        AA_BODY,
      );
    }
  });

  it('still clears AA when the system material shows a white desktop through', () => {
    // The worst case there is: nothing behind the window is brighter than white,
    // so a floor that holds here holds everywhere.
    //
    // Only `ink-0` and `ink-1` are checked, because only those two are allowed on
    // the ground — see the test below. The arithmetic is unforgiving: `ink-2`
    // needs the ground at ~97% before it clears AA over white, which is opaque in
    // all but name. Rather than ship a material nobody can see, or text nobody
    // can read, the rule is that the dimmer inks live on opaque panels.
    const ground = composite(token('ground-0'), '#FFFFFF', groundOpacity());

    for (const ink of ['ink-0', 'ink-1']) {
      expect(
        contrast(token(ink), ground),
        `${ink} over a white desktop`,
      ).toBeGreaterThanOrEqual(AA_BODY);
    }
  });

  it('keeps the material a depth cue rather than a wash', () => {
    expect(groundOpacity()).toBeGreaterThanOrEqual(0.85);
  });

  it('puts no dim ink directly on the ground', () => {
    // Panels (`Section`, `Row`) paint their own opaque background, so `ink-2` and
    // `ink-3` are safe inside them. These two surfaces render straight onto the
    // window ground, where the material can lighten it, and may not use them.
    const onTheGround = {
      'Projects.tsx': projects,
      'ProjectDetail.tsx': projectDetail,
    };

    for (const [name, source] of Object.entries(onTheGround)) {
      for (const dim of ['text-ink-2', 'text-ink-3']) {
        expect(source, `${name} puts ${dim} on the window ground`).not.toContain(dim);
      }
    }
  });
});
