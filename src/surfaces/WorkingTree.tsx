import type { Project } from '../bindings/Project';
import { Button } from '../components/Button';
import { Changes } from '../components/Changes';
import { Icon } from '../components/Icon';

/**
 * What you have changed and not committed.
 *
 * A **separate surface from History**, deliberately. A working tree is not a
 * commit: one is what you have done and the other is what is recorded, and a
 * single list of both would make it impossible to tell them apart. Mira answers
 * them with two different scopes and shows them in two different places
 * ([ADR-0016](../../docs/adr/0016-bounded-diffs.md)).
 *
 * **Read-only, like everything else.** Nothing here stages, discards, checks out
 * or reverts — and there is no greyed-out control implying that one is coming.
 * The absence is part of the design (`information-architecture.md` §5).
 */
export function WorkingTree({ project, onBack }: { project: Project; onBack: () => void }) {
  return (
    <div className="flex min-w-0 flex-col gap-[var(--section-gap)]">
      <header className="flex flex-col gap-[var(--space-2)]">
        <button
          type="button"
          onClick={onBack}
          aria-label={`Back to ${project.name}`}
          className="t-ui flex w-fit cursor-default items-center gap-[var(--space-1)] text-ink-1 hover:text-ink-0"
        >
          <Icon name="back" />
          {project.name}
        </button>
        <h1 className="t-value-lg m-0 text-ink-0">Working tree</h1>
        <p className="t-ui m-0 text-ink-1">
          What is on disk and not yet in a commit, staged or not.
        </p>
      </header>

      <Changes projectId={project.id} scope={{ kind: 'workingTree' }} label="Changed files" />

      <div>
        <Button onClick={onBack}>Back to the project</Button>
      </div>

      <p className="t-ui m-0 text-ink-1">
        Mira reads your working tree and never changes it. There is nothing here that stages,
        discards, checks out or reverts.
      </p>
    </div>
  );
}
