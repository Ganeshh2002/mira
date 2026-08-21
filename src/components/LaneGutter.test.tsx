import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { GraphRow } from '../bindings/GraphRow';
import { LaneGutter } from './LaneGutter';

/**
 * The gutter draws lines. These assert *which* lines, because the one thing a
 * graph must never do is draw a connection that is not there — and the one place
 * that could happen is here, between a layout the backend computed and an SVG.
 */

const commit = {
  sha: 'a'.repeat(40),
  shortSha: 'aaaaaaa',
  subject: 'a commit',
  author: 'Ganeshh',
  committedAt: 1_800_000_000,
};

function row(overrides: Partial<GraphRow> = {}): GraphRow {
  return {
    commit,
    parents: [],
    lane: 0,
    kind: 'normal',
    refs: [],
    edges: [],
    continuing: [],
    ...overrides,
  };
}

function draw(given: GraphRow, above: number[] = [], lanes = 2) {
  const { container } = render(<LaneGutter row={given} above={above} lanes={lanes} />);
  return {
    lines: container.querySelectorAll('line'),
    curves: container.querySelectorAll('path'),
    circles: container.querySelectorAll('circle'),
    svg: container.querySelector('svg'),
  };
}

describe('the lane gutter', () => {
  it('is decoration, and says so', () => {
    // Every fact it draws is written on the row beside it. A screen reader that
    // announced the picture would be reading it out twice.
    const { svg } = draw(row());

    expect(svg).toHaveAttribute('aria-hidden', 'true');
    expect(svg).not.toHaveAttribute('role');
  });

  it('meets the row above and the row below', () => {
    // A line drawn to the bottom of one row has to meet the line drawn from the
    // top of the next, without either knowing the other exists.
    const { lines } = draw(row({ continuing: [0] }), [0]);

    expect(lines).toHaveLength(2);
    expect(lines[0]).toHaveAttribute('y1', '0');
    expect(lines[0]).toHaveAttribute('y2', '24');
    expect(lines[1]).toHaveAttribute('y1', '24');
    expect(lines[1]).toHaveAttribute('y2', '48');
  });

  it('draws nothing above the first row', () => {
    const { lines } = draw(row({ continuing: [0] }), []);

    expect(lines).toHaveLength(1);
    expect(lines[0]).toHaveAttribute('y1', '24');
  });

  it('draws no line at all for a root commit', () => {
    const { lines, curves } = draw(row({ kind: 'root' }), []);

    expect(lines).toHaveLength(0);
    expect(curves).toHaveLength(0);
  });

  it('curves only where a line changes lane', () => {
    // An edge that stays in the commit's own lane is already the vertical below
    // it; drawing both would double the stroke and mean nothing extra.
    const { curves } = draw(
      row({
        lane: 0,
        kind: 'merge',
        edges: [
          { to: 0, kind: 'straight' },
          { to: 1, kind: 'merge' },
        ],
        continuing: [0, 1],
      }),
      [0],
    );

    expect(curves).toHaveLength(1);
  });

  it('draws nothing for a parent that sits above the row', () => {
    // The honest answer to a history whose dates disagree with its topology: the
    // relationship is reported by the backend and drawn by nobody, because a line
    // running upward into a row already scrolled past is a picture of nothing.
    const { curves } = draw(row({ lane: 1, edges: [{ to: 1, kind: 'reordered' }] }), [1]);

    expect(curves).toHaveLength(0);
  });

  it('marks a merge with a shape rather than only a colour', () => {
    const ordinary = draw(row()).circles;
    const merge = draw(row({ kind: 'merge' })).circles;

    expect(ordinary).toHaveLength(1);
    expect(merge).toHaveLength(2);
  });

  it('fills the node of the first commit in the history', () => {
    const { circles } = draw(row({ kind: 'root' }));

    expect(circles).toHaveLength(1);
    expect(circles[0]).toHaveAttribute('fill', 'var(--lane-0)');
  });

  it('folds every lane past the eighth onto the last colour', () => {
    // The ramp has eight steps, and a lane index past it must not reach for a
    // token that does not exist.
    const { circles } = draw(row({ lane: 12 }), [], 13);

    expect(circles[0]).toHaveAttribute('stroke', 'var(--lane-7)');
  });

  it('is as wide as the page needs and no wider', () => {
    expect(draw(row(), [], 1).svg).toHaveAttribute('width', '14');
    expect(draw(row(), [], 4).svg).toHaveAttribute('width', '56');
  });
});
