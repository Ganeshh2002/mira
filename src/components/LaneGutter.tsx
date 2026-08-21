import type { GraphRow } from '../bindings/GraphRow';

/**
 * The lane gutter beside one commit row.
 *
 * Named for what it is — the strip of lines to the left of a row — rather than
 * for the graph as a whole, which is the `CommitGraph` the backend sends.
 *
 * **Decorative, and honestly so.** Every fact this draws is also written on the
 * row beside it — that a commit is a merge, which branch points at it, what it
 * says. So the SVG is `aria-hidden` and carries no label: a screen reader that
 * announced "lane 2, two edges" would be reading out a picture rather than the
 * thing it is a picture of (`design-system.md` §10, and the rule that colour and
 * shape always have a text partner).
 *
 * The geometry is fixed rather than measured. Each row is exactly one
 * `--graph-row` tall and each lane one `--graph-lane` wide, so a line drawn to the
 * bottom of one row meets the line drawn from the top of the next without either
 * knowing the other exists. That is what lets the gutter be a per-row SVG instead
 * of one tall canvas the list would have to keep in step with.
 */

/** Row height and lane width, read from the tokens rather than assumed. */
const ROW = 48;
const LANE = 14;
const NODE = 3.5;

/** The centre of a lane, in the row's own coordinates. */
function centre(lane: number): number {
  return lane * LANE + LANE / 2;
}

/** The muted eight-step ramp, by lane index (`design-system.md` §8). */
function stroke(lane: number): string {
  return `var(--lane-${Math.min(lane, 7)})`;
}

export function LaneGutter({
  row,
  above,
  lanes,
}: {
  row: GraphRow;
  /** The lanes occupied by the row above, so its lines meet this one's. */
  above: number[];
  /** How many lanes the whole page uses, so every gutter is the same width. */
  lanes: number;
}) {
  const width = Math.max(lanes, 1) * LANE;
  const half = ROW / 2;
  const x = centre(row.lane);

  return (
    <svg
      width={width}
      height={ROW}
      viewBox={`0 0 ${width} ${ROW}`}
      aria-hidden="true"
      focusable="false"
      className="shrink-0"
      style={{ width: `calc(${Math.max(lanes, 1)} * var(--graph-lane))` }}
    >
      {/* Lines arriving from the row above, meeting this row's centre. */}
      {above.map((lane) => (
        <line
          key={`in-${lane}`}
          x1={centre(lane)}
          y1={0}
          x2={centre(lane)}
          y2={half}
          stroke={stroke(lane)}
          strokeWidth={1}
        />
      ))}

      {/* Lines carrying on below, which the next row's gutter will meet. */}
      {row.continuing.map((lane) => (
        <line
          key={`out-${lane}`}
          x1={centre(lane)}
          y1={half}
          x2={centre(lane)}
          y2={ROW}
          stroke={stroke(lane)}
          strokeWidth={1}
        />
      ))}

      {/*
        A line that changes lane: a branch bending back into the mainline, or a
        merge reaching sideways for its second parent. An edge that stays in this
        commit's own lane is already drawn by the vertical above, and a
        `reordered` edge is drawn by nothing — its parent is above this row, and
        a line running upward into a row already scrolled past would be a picture
        of something that is not there.
      */}
      {row.edges
        .filter((edge) => edge.kind !== 'reordered' && edge.to !== row.lane)
        .map((edge) => (
          <path
            key={`edge-${edge.to}-${edge.kind}`}
            d={`M ${x} ${half} C ${x} ${ROW - 6}, ${centre(edge.to)} ${half + 6}, ${centre(
              edge.to,
            )} ${ROW}`}
            fill="none"
            stroke={stroke(edge.kind === 'merge' ? edge.to : row.lane)}
            strokeWidth={1}
          />
        ))}

      <Node kind={row.kind} x={x} y={half} lane={row.lane} />
    </svg>
  );
}

/**
 * The dot on the row.
 *
 * Three shapes, not three colours: a ring for an ordinary commit, a ring with a
 * filled centre for a merge, a solid disc for the first commit in the history.
 * The shape is the second channel; the word is on the row itself
 * (`design-system.md` §5 — a status never depends on hue alone).
 */
function Node({
  kind,
  x,
  y,
  lane,
}: {
  kind: GraphRow['kind'];
  x: number;
  y: number;
  lane: number;
}) {
  const colour = stroke(lane);

  if (kind === 'root') {
    return <circle cx={x} cy={y} r={NODE} fill={colour} />;
  }

  return (
    <>
      <circle cx={x} cy={y} r={NODE} fill="var(--ground-1)" stroke={colour} strokeWidth={1.5} />
      {kind === 'merge' ? <circle cx={x} cy={y} r={1.25} fill={colour} /> : null}
    </>
  );
}
