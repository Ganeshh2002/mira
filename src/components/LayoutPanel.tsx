import type { MonorepoTool } from '../bindings/MonorepoTool';
import type { RepositoryLayout } from '../bindings/RepositoryLayout';
import { Row } from './Row';
import { Section } from './Section';

/**
 * How the project sits in the repository around it.
 *
 * Absent when there is nothing to say — a standalone repository renders no
 * section at all, rather than a section saying "not a monorepo"
 * (`design-system.md` §8: a section with nothing to say is absent, never empty).
 *
 * Packages listed here are boundaries, not projects. Mira has not adopted them,
 * and clicking one does nothing yet: turning a detected package into a project
 * the user has to manage is their decision, not a side effect of looking.
 */
export function LayoutPanel({ layout }: { layout: RepositoryLayout }) {
  if (layout.kind === 'standalone') return null;

  if (layout.kind === 'package') {
    return (
      <Section label="Monorepo">
        <Row label="Repository" value={layout.monorepoRoot} detail={tools(layout.tools)} />
        <Row label="Package" value={layout.packagePath} detail={layout.packageName} />
      </Section>
    );
  }

  return (
    <>
      <Section label="Monorepo">
        <Row label="Workspace" value={tools(layout.tools)} />
      </Section>

      {layout.packages.length > 0 ? (
        <section className="flex flex-col gap-[var(--space-2)]">
          <h2 className="t-label text-ink-1">Packages</h2>
          <ul
            aria-label="Packages"
            className="m-0 flex list-none flex-col overflow-hidden rounded-md border border-line bg-ground-1 p-0"
          >
            {layout.packages.map((pkg) => (
              <li
                key={pkg.path}
                className="flex min-h-[var(--row-height)] items-center gap-[var(--space-3)] border-b border-line px-[var(--space-3)] py-[var(--space-2)] last:border-b-0"
              >
                <span className="t-ui min-w-0 shrink truncate text-ink-0">{pkg.name}</span>
                <span className="t-value ml-auto min-w-0 truncate text-ink-2">{pkg.path}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </>
  );
}

/** The tools that declare the workspace, as one line. */
function tools(found: MonorepoTool[]): string {
  return found.map(label).join(' · ');
}

/**
 * Each tool written the way it writes its own name.
 *
 * The backend sends the variant; the wording lives here because it is interface
 * copy. `npm` is lower case because that is its name, not a slip.
 */
function label(tool: MonorepoTool): string {
  switch (tool) {
    case 'npmWorkspaces':
      return 'npm workspaces';
    case 'pnpmWorkspaces':
      return 'pnpm workspaces';
    case 'yarnWorkspaces':
      return 'Yarn workspaces';
    case 'cargoWorkspace':
      return 'Cargo workspace';
    case 'turborepo':
      return 'Turborepo';
    case 'nx':
      return 'Nx';
  }
}
