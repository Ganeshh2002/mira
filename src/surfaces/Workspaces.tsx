import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';

import type { Project } from '../bindings/Project';
import type { Workspace } from '../bindings/Workspace';
import { Button } from '../components/Button';
import { commands, describeUnknown } from '../lib/ipc';
import { workspaceKeys } from '../lib/workspaces';

/**
 * A project's workspaces, and the form that adds one.
 *
 * A workspace needs a name and nothing else. Packages, applications and services
 * are not asked for at creation, because a workspace is useful the moment it
 * exists: everything it shows comes from the project underneath it.
 */
export function WorkspaceList({
  project,
  selected,
  onSelect,
}: {
  project: Project;
  selected: number | null;
  onSelect: (workspace: Workspace) => void;
}) {
  const client = useQueryClient();
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');

  const workspaces = useQuery({
    queryKey: workspaceKeys.of(project.id),
    queryFn: () => commands.workspacesList(project.id),
  });

  const create = useMutation({
    mutationFn: () =>
      commands.workspacesCreate(project.id, name.trim(), description.trim() || null),
    onSuccess: async (made) => {
      await client.invalidateQueries({ queryKey: workspaceKeys.of(project.id) });
      setNaming(false);
      setName('');
      setDescription('');
      onSelect(made);
    },
  });

  if (workspaces.isPending) {
    return <p className="t-ui text-ink-1">Reading workspaces…</p>;
  }

  if (workspaces.isError) {
    return (
      <p role="alert" className="t-ui text-signal-danger">
        {describeUnknown(workspaces.error)}
      </p>
    );
  }

  return (
    <section className="flex flex-col gap-[var(--space-2)]">
      <h2 className="t-label text-ink-1">Workspaces</h2>

      {workspaces.data.length === 0 ? (
        <p className="t-ui m-0 text-ink-1">No workspaces yet.</p>
      ) : (
        <ul
          aria-label="Workspaces"
          className="m-0 flex list-none flex-col gap-[var(--space-1)] p-0"
        >
          {workspaces.data.map((workspace) => (
            <li key={workspace.id}>
              <button
                type="button"
                onClick={() => onSelect(workspace)}
                aria-current={workspace.id === selected ? 'true' : undefined}
                className={`t-ui flex w-full cursor-default flex-col items-start rounded-sm px-[var(--space-2)] py-[var(--space-1)] text-left transition-colors duration-[var(--motion-instant)] ${
                  workspace.id === selected
                    ? 'bg-ember-wash text-ink-0'
                    : 'text-ink-1 hover:bg-ground-2'
                }`}
              >
                <span className="min-w-0 truncate">{workspace.name}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      {naming ? (
        <form
          className="flex flex-col gap-[var(--space-2)] rounded-md border border-line bg-ground-1 p-[var(--space-3)]"
          onSubmit={(event) => {
            event.preventDefault();
            // Refused here as well as in Rust: a form that submits nothing and
            // waits for a round trip to say "needs a name" is slower and no
            // clearer.
            if (name.trim()) create.mutate();
          }}
        >
          <label className="t-ui flex flex-col gap-[var(--space-1)] text-ink-1">
            Name
            <input
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              className="t-ui rounded-sm border border-line bg-ground-2 px-[var(--space-2)] py-[var(--space-1)] text-ink-0"
            />
          </label>

          <label className="t-ui flex flex-col gap-[var(--space-1)] text-ink-1">
            Description
            <input
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              placeholder="Optional"
              className="t-ui rounded-sm border border-line bg-ground-2 px-[var(--space-2)] py-[var(--space-1)] text-ink-0"
            />
          </label>

          <div className="flex gap-[var(--space-2)]">
            <Button kind="accent" type="submit">
              Create
            </Button>
            <Button
              onClick={() => {
                setNaming(false);
                create.reset();
              }}
            >
              Cancel
            </Button>
          </div>

          {create.error ? (
            <p role="alert" className="t-ui m-0 text-signal-danger">
              {describeUnknown(create.error)}
            </p>
          ) : null}
        </form>
      ) : (
        <Button onClick={() => setNaming(true)}>New workspace</Button>
      )}
    </section>
  );
}
