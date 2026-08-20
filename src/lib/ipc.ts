import { invoke } from '@tauri-apps/api/core';

import type { FoundationStatus } from '../bindings/FoundationStatus';
import type { MiraError } from '../bindings/MiraError';
import type { Project } from '../bindings/Project';
import type { ProjectContext } from '../bindings/ProjectContext';

/**
 * The typed IPC client.
 *
 * The frontend has no privileges of its own (architecture.md §7): no fs, no shell,
 * no http, no SQL. Everything it can do is a named command below, and every type
 * crossing this boundary is generated from Rust by ts-rs — none is hand-written.
 */

/** A command that failed, carrying the structured reason the backend gave. */
export class MiraCommandError extends Error {
  readonly detail: MiraError;

  constructor(detail: MiraError) {
    super(describeError(detail));
    this.name = 'MiraCommandError';
    this.detail = detail;
  }
}

/**
 * Errors state the fact and the fix — never an apology, never a stack trace as the
 * first thing (design-system §9).
 */
export function describeError(error: MiraError): string {
  switch (error.kind) {
    case 'notFound':
      return `${error.what} was not found.`;
    case 'permissionDenied':
      return `${error.what} was refused. ${error.hint}`;
    case 'unsupported':
      return error.reason;
    case 'timeout':
      return `${error.operation} did not finish within ${error.afterMs} ms.`;
    case 'external':
      return `${error.source} failed. ${error.detail}`;
    case 'invalid':
      // `field` says which input failed, for code that needs to know. Showing it
      // turns a clear sentence into "path is not valid. …" — form-validation
      // language in a product with no form. The detail is the whole message.
      return error.detail;
  }
}

/**
 * Anything thrown, as one sentence for a person.
 *
 * Errors state the fact and the fix; a raw exception never reaches the screen
 * (design-system §9).
 */
export function describeUnknown(error: unknown): string {
  if (error instanceof MiraCommandError) return error.message;
  if (isMiraError(error)) return describeError(error);
  return error instanceof Error ? error.message : String(error);
}

function isMiraError(value: unknown): value is MiraError {
  return typeof value === 'object' && value !== null && 'kind' in value;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    if (isMiraError(raw)) {
      throw new MiraCommandError(raw);
    }
    throw new MiraCommandError({
      kind: 'external',
      source: 'Mira',
      detail: raw instanceof Error ? raw.message : String(raw),
    });
  }
}

/** Every command the frontend may invoke. This list is the privilege surface. */
export const commands = {
  /** `app.foundation_status` — the shell's own state. */
  foundationStatus: (): Promise<FoundationStatus> => call('app_foundation_status'),
  /** `app.open_settings` — reveal the Settings window. */
  openSettings: (): Promise<void> => call('app_open_settings'),

  /** `projects.list` — every project, most recently opened first. */
  projectsList: (): Promise<Project[]> => call('projects_list'),

  /**
   * `projects.add` — register a folder the user picks.
   *
   * No argument, deliberately: the folder is chosen by a native dialog in Rust,
   * so the interface has no path to send and cannot invent one. `null` means the
   * picker was dismissed, which is an outcome rather than an error.
   */
  projectsAdd: (): Promise<Project | null> => call('projects_add'),

  /** `projects.open` — record that a project was opened. */
  projectsOpen: (projectId: number): Promise<Project> => call('projects_open', { projectId }),

  /** `projects.remove` — forget a project. The folder on disk is untouched. */
  projectsRemove: (projectId: number): Promise<void> => call('projects_remove', { projectId }),

  /** `projects.reveal` — open a project's folder in the file manager. */
  projectsReveal: (projectId: number): Promise<void> => call('projects_reveal', { projectId }),

  /**
   * `projects.context` — the project's Git state and repository layout, read on
   * demand.
   */
  projectsContext: (projectId: number): Promise<ProjectContext> =>
    call('projects_context', { projectId }),
};
