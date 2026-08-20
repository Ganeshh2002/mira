import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import type { Project } from '../bindings/Project';
import { Button } from '../components/Button';
import { commands, describeUnknown } from '../lib/ipc';
import { ProjectDetail } from './ProjectDetail';

/**
 * The root of the main window (`information-architecture.md` §3): the project
 * list is always present, and there is no mode that hides the other projects.
 *
 * Selection lives here rather than in the detail view, so switching projects is a
 * state change and not a reload — each project's context is cached under its own
 * query key and comes back untouched.
 */
export function Projects() {
  const client = useQueryClient();
  const [selected, setSelected] = useState<number | null>(null);

  const projects = useQuery({
    queryKey: ['projects'],
    queryFn: commands.projectsList,
  });

  const add = useMutation({
    mutationFn: commands.projectsAdd,
    onSuccess: async (project) => {
      await client.invalidateQueries({ queryKey: ['projects'] });
      // `null` means the picker was dismissed. Nothing changed, so nothing is
      // said — an empty dialog is not an error to report.
      if (project) setSelected(project.id);
    },
  });

  const open = useMutation({
    mutationFn: commands.projectsOpen,
    onSuccess: () => client.invalidateQueries({ queryKey: ['projects'] }),
  });

  if (projects.isPending) {
    return <p className="t-ui text-ink-1">Reading your projects…</p>;
  }

  if (projects.isError) {
    return (
      <p role="alert" className="t-body text-signal-danger">
        {describeUnknown(projects.error)}
      </p>
    );
  }

  const list = projects.data;
  const current = list.find((project) => project.id === selected) ?? list[0];

  function choose(project: Project) {
    // A refusal from the last add is about a folder, not about this project. It
    // stops being information the moment attention moves.
    add.reset();
    setSelected(project.id);
    open.mutate(project.id);
    // FR-3.3's first refresh trigger. Coming back to a project is a statement
    // that you want its state *now*; without this the panel would show whatever
    // was true when you last looked, because nothing here polls.
    void client.invalidateQueries({ queryKey: ['git', 'context', project.id] });
  }

  return (
    <div className="flex min-h-0 flex-1 gap-[var(--space-6)]">
      <nav className="flex w-[220px] shrink-0 flex-col gap-[var(--space-3)]">
        <h2 className="t-label text-ink-1">Projects</h2>

        {list.length === 0 ? (
          <p className="t-ui m-0 text-ink-1">No projects yet.</p>
        ) : (
          <ul
            aria-label="Projects"
            className="m-0 flex list-none flex-col gap-[var(--space-1)] p-0"
          >
            {list.map((project) => (
              <li key={project.id}>
                <ProjectButton
                  project={project}
                  selected={project.id === current?.id}
                  onSelect={() => choose(project)}
                />
              </li>
            ))}
          </ul>
        )}

        <Button kind={list.length === 0 ? 'accent' : 'quiet'} onClick={() => add.mutate()}>
          Add Project
        </Button>

        {add.error ? (
          <p role="alert" className="t-ui m-0 text-signal-danger">
            {describeUnknown(add.error)}
          </p>
        ) : null}
      </nav>

      <div className="min-w-0 flex-1">
        {current ? (
          // Keyed by project, so switching remounts the detail view. Without it
          // the previous project's failed action and its half-finished removal
          // prompt would still be on screen, attached to a different project.
          <ProjectDetail
            key={current.id}
            project={current}
            onRemoved={() => setSelected(null)}
          />
        ) : (
          <EmptyState />
        )}
      </div>
    </div>
  );
}

function ProjectButton({
  project,
  selected,
  onSelect,
}: {
  project: Project;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-current={selected ? 'true' : undefined}
      className={`t-ui flex w-full cursor-default items-center gap-[var(--space-2)] rounded-sm px-[var(--space-2)] py-[var(--space-1)] text-left transition-colors duration-[var(--motion-instant)] ${
        selected ? 'bg-ember-wash text-ink-0' : 'text-ink-1 hover:bg-ground-2'
      }`}
    >
      <span
        aria-hidden="true"
        className={`shrink-0 ${project.isGit ? 'text-ember-dim' : 'text-ink-1'}`}
      >
        {project.isGit ? '●' : '○'}
      </span>
      <span className="min-w-0 truncate">{project.name}</span>
    </button>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col gap-[var(--space-2)]">
      <p className="t-body m-0 text-ink-1">Point Mira at a folder you work in.</p>
      <p className="t-ui m-0 text-ink-1">
        Mira reads what is there — the repository, the branch, the last commit — and never
        changes it.
      </p>
    </div>
  );
}
