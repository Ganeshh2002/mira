import { invoke } from '@tauri-apps/api/core';

import type { FoundationStatus } from '../bindings/FoundationStatus';
import type { MiraError } from '../bindings/MiraError';
import type { AppKind } from '../bindings/AppKind';
import type { AppId } from '../bindings/AppId';
import type { AppReport } from '../bindings/AppReport';
import type { Catalogue } from '../bindings/Catalogue';
import type { ChosenApp } from '../bindings/ChosenApp';
import type { ChangedFiles } from '../bindings/ChangedFiles';
import type { CommitGraph } from '../bindings/CommitGraph';
import type { CommitId } from '../bindings/CommitId';
import type { CommitLookup } from '../bindings/CommitLookup';
import type { CommitPage } from '../bindings/CommitPage';
import type { DiffScope } from '../bindings/DiffScope';
import type { FileDiff } from '../bindings/FileDiff';
import type { FileHistory } from '../bindings/FileHistory';
import type { FileSubject } from '../bindings/FileSubject';
import type { FilteredHistory } from '../bindings/FilteredHistory';
import type { HistoryFilter } from '../bindings/HistoryFilter';
import type { KnownAuthors } from '../bindings/KnownAuthors';
import type { KnownRefs } from '../bindings/KnownRefs';
import type { KeepAwakeSpan } from '../bindings/KeepAwakeSpan';
import type { KeepAwakeState } from '../bindings/KeepAwakeState';
import type { Launched } from '../bindings/Launched';
import type { ShaForm } from '../bindings/ShaForm';
import type { LiveSnapshot } from '../bindings/LiveSnapshot';
import type { Project } from '../bindings/Project';
import type { ServiceOffer } from '../bindings/ServiceOffer';
import type { ActionId } from '../bindings/ActionId';
import type { ActionOffer } from '../bindings/ActionOffer';
import type { Performed } from '../bindings/Performed';
import type { PortsView } from '../bindings/PortsView';
import type { Workspace } from '../bindings/Workspace';
import type { WorkspaceAction } from '../bindings/WorkspaceAction';
import type { WorkspaceService } from '../bindings/WorkspaceService';

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

  /** `live.snapshot` — what the observers last saw. A read of memory. */
  liveSnapshot: (): Promise<LiveSnapshot> => call('live_snapshot'),

  /** `live.refresh` — observe now rather than at the next tick. */
  liveRefresh: (): Promise<LiveSnapshot> => call('live_refresh'),

  /**
   * `live.open_service` — open one of the observed services in the browser.
   *
   * A **position** in `live.snapshot`'s service list — not a port, and not a
   * URL. Mira builds `http://localhost:<port>` in Rust from its own observation,
   * so there is no argument here through which a page could ask it to open a
   * `file://` path or a port nobody is serving
   * (`security-and-privacy.md` §5 rule 5).
   */
  openService: (at: number): Promise<void> => call('live_open_service', { at }),

  /**
   * `live.ports` — every listening socket on this machine, grouped.
   *
   * Machine-scoped, and the only place Mira shows machine-wide data. A read of
   * the observation the scheduler already took, arranged — it starts no observer
   * and takes no reading of its own.
   *
   * Rows carry a port so they can be recognised; opening one still goes through
   * `openService(at)`, a position in the list, so no port travels inward.
   */
  ports: (): Promise<PortsView> => call('live_ports'),

  /**
   * `git.history` — one page of a project's repository history.
   *
   * `cursor` is the `next` the previous page returned; `null` starts at HEAD.
   * There is no page-size argument, deliberately: the page is Mira's
   * (`mira_git::PAGE`), so the interface cannot ask for a whole repository and a
   * guard test fails the build if a `limit` ever appears.
   */
  gitHistory: (projectId: number, cursor: CommitId | null): Promise<CommitPage> =>
    call('git_history', { projectId, cursor }),

  /**
   * `git.graph` — the same page as `git.history`, with the shape of it.
   *
   * Parents, lanes and reference labels, computed over that page and nothing
   * else. The same cursor and the same bound: still no way to say how much to
   * read, so a page costs a page whatever the repository behind it.
   *
   * A **picture**. Nothing reachable from here checks out, merges, rebases,
   * resets, cherry-picks or touches a remote.
   */
  gitGraph: (projectId: number, cursor: CommitId | null): Promise<CommitGraph> =>
    call('git_graph', { projectId, cursor }),

  /** `git.commit` — one commit, in the detail its own view shows. Read-only. */
  gitCommit: (projectId: number, commit: CommitId): Promise<CommitLookup> =>
    call('git_commit', { projectId, commit }),

  /**
   * `git.copy_commit` — put a commit id on the clipboard.
   *
   * A commit and a spelling, never the text itself. Mira resolves the commit in
   * the repository and copies what came back, so this cannot be used to place a
   * string of the interface's choosing on somebody's clipboard. Returns what was
   * copied, so the interface can confirm it without keeping its own idea of what
   * the clipboard holds.
   */
  gitCopyCommit: (projectId: number, commit: CommitId, form: ShaForm): Promise<string> =>
    call('git_copy_commit', { projectId, commit, form }),

  /**
   * `git.changes` — what a commit changed, or what the working tree has.
   *
   * The scope is a commit or the working tree, and they stay separate answers.
   * Bounded by `MAX_FILES`; the reply says when the bound bit.
   */
  gitChanges: (projectId: number, scope: DiffScope): Promise<ChangedFiles> =>
    call('git_changes', { projectId, scope }),

  /**
   * `git.file_diff` — one file's patch.
   *
   * `at` is the file's **position** in the list `gitChanges` returned, never a
   * path. The interface can only ask for a file Mira already decided to offer,
   * and an ordinal past the list reads nothing.
   */
  gitFileDiff: (projectId: number, scope: DiffScope, at: number): Promise<FileDiff> =>
    call('git_file_diff', { projectId, scope, at }),

  /**
   * `git.file_history` — the commits that touched one file.
   *
   * The file is named by a `subject` the interface **received** — from a change
   * list, or from a previous page's cursor — and hands back unchanged. There is
   * no path here and no way to build one: a subject is a change set Mira produced
   * and a position in it.
   *
   * Bounded by commits examined rather than by anything about the file, because
   * looking is the cost. A page that ran out of budget says how far it got.
   */
  gitFileHistory: (
    projectId: number,
    subject: FileSubject,
    cursor: string | null,
  ): Promise<FileHistory> => call('git_file_history', { projectId, subject, cursor }),

  /**
   * `git.search` — the commits matching a filter.
   *
   * Composable: branch, author, subject and file narrow together. Nothing in the
   * filter is a Git argument — a branch is a **commit id** the interface was
   * given, a file is a change-set position, and author and subject are text that
   * is only ever compared in Rust.
   *
   * Bounded by commits examined. A search that ran out of budget says so, and
   * that is a different answer from one that found nothing.
   */
  gitSearch: (
    projectId: number,
    wanted: HistoryFilter,
    cursor: string | null,
  ): Promise<FilteredHistory> => call('git_search', { projectId, wanted, cursor }),

  /**
   * `git.refs` — the branches and tags a search may start from.
   *
   * Each carries the commit it points at, and that is what a filter sends back.
   * The name is for reading; the tip is for asking.
   */
  gitRefs: (projectId: number): Promise<KnownRefs> => call('git_refs', { projectId }),

  /** `git.authors` — the authors of the commits within one scan budget. */
  gitAuthors: (projectId: number): Promise<KnownAuthors> => call('git_authors', { projectId }),

  /** `keep_awake.state` — whether the machine is being kept awake, and until when. */
  keepAwakeState: (): Promise<KeepAwakeState> => call('keep_awake_state'),

  /**
   * `keep_awake.set` — hold one of four spans, or none of them.
   *
   * One word out of four. The interface cannot name a duration, so there is no
   * number here for anything downstream to bound (ADR-0014).
   */
  keepAwakeSet: (span: KeepAwakeSpan): Promise<KeepAwakeState> =>
    call('keep_awake_set', { span }),

  /** `workspaces.list` — one project's workspaces, most recently opened first. */
  workspacesList: (projectId: number): Promise<Workspace[]> =>
    call('workspaces_list', { projectId }),

  /** `workspaces.create` — a name and a project is all it takes. */
  workspacesCreate: (
    projectId: number,
    name: string,
    description: string | null,
  ): Promise<Workspace> => call('workspaces_create', { projectId, name, description }),

  /** `workspaces.rename` — change the name and description. */
  workspacesRename: (
    workspaceId: number,
    name: string,
    description: string | null,
  ): Promise<Workspace> => call('workspaces_rename', { workspaceId, name, description }),

  /**
   * `workspaces.open` — make a workspace the one being worked in.
   *
   * Records when, and nothing else. Opening does not launch an editor or start a
   * server; the project's context is already being observed.
   */
  workspacesOpen: (workspaceId: number): Promise<Workspace> =>
    call('workspaces_open', { workspaceId }),

  /** `workspaces.remove` — forget a workspace. Its project is untouched. */
  workspacesRemove: (workspaceId: number): Promise<void> =>
    call('workspaces_remove', { workspaceId }),

  /** `workspaces.set_applications` — the whole list of kinds, every time. */
  workspacesSetApplications: (workspaceId: number, kinds: AppKind[]): Promise<Workspace> =>
    call('workspaces_set_applications', { workspaceId, kinds }),

  /**
   * `workspaces.catalogue` — every application Mira knows to look for, for one
   * kind, with what is on this machine marked.
   *
   * The menu a choice is made from. Because it is **a list Mira produced**, a
   * choice can be sent back as the identity it came with; the interface has no
   * way to name an application it was not offered.
   */
  workspacesCatalogue: (kind: AppKind): Promise<Catalogue> =>
    call('workspaces_catalogue', { kind }),

  /**
   * `workspaces.prefer` — which application this workspace uses for one kind.
   *
   * `null` goes back to automatic. The id names a row in Mira's own catalogue —
   * never a path, a program name or a command — and one that names no row is
   * refused rather than stored.
   */
  workspacesPrefer: (
    workspaceId: number,
    kind: AppKind,
    application: AppId | null,
  ): Promise<Workspace> => call('workspaces_prefer', { workspaceId, kind, application }),

  /**
   * `workspaces.chosen` — what this workspace's choice resolves to here.
   *
   * Asked before a button is offered, so an application uninstalled since it was
   * chosen is a sentence on the row rather than an error after a click.
   */
  workspacesChosen: (workspaceId: number, kind: AppKind): Promise<ChosenApp> =>
    call('workspaces_chosen', { workspaceId, kind }),

  /** `workspaces.applications` — what this machine actually has. */
  workspacesApplications: (): Promise<AppReport[]> => call('workspaces_applications'),

  /**
   * `workspaces.openable` — the kinds this machine can open a folder in.
   *
   * Narrower than `workspacesApplications`, and deliberately a second question:
   * a machine can *have* an editor Mira has no way to open a directory in.
   */
  workspacesOpenable: (): Promise<AppReport[]> => call('workspaces_openable'),

  /**
   * `workspaces.launch` — open this workspace's project in an application.
   *
   * A workspace and a kind. Not a path, not a program, not a command line, and
   * not an address: the directory is resolved in Rust from the project row, and
   * the application from a table compiled into the binary. This signature is the
   * entire privilege the interface has to start anything
   * (`security-and-privacy.md` §5 rule 1).
   */
  workspacesLaunch: (workspaceId: number, kind: AppKind): Promise<Launched> =>
    call('workspaces_launch', { workspaceId, kind }),

  /**
   * `workspaces.services` — what this workspace watches, and where each stands.
   *
   * Resolved on every call against the reading the observers already took. A
   * service that has stopped comes back as its port and a state, never as a
   * remembered copy of what used to be listening there.
   */
  workspacesServices: (workspaceId: number): Promise<WorkspaceService[]> =>
    call('workspaces_services', { workspaceId }),

  /**
   * `workspaces.service_offers` — the project's services, as things to add.
   *
   * The list `at` indexes. It carries ports outward so a person can recognise
   * what they are picking; the way back is a position in it.
   */
  workspacesServiceOffers: (workspaceId: number): Promise<ServiceOffer[]> =>
    call('workspaces_service_offers', { workspaceId }),

  /**
   * `workspaces.watch_service` — start watching one of the project's services.
   *
   * `at` is a position in the offer list. There is no port here: the interface
   * can ask for a service Mira already decided to offer, and nothing else.
   */
  workspacesWatchService: (workspaceId: number, at: number): Promise<WorkspaceService[]> =>
    call('workspaces_watch_service', { workspaceId, at }),

  /** `workspaces.forget_service` — stop watching one, by the id Mira issued. */
  workspacesForgetService: (
    workspaceId: number,
    serviceId: number,
  ): Promise<WorkspaceService[]> =>
    call('workspaces_forget_service', { workspaceId, serviceId }),

  /**
   * `workspaces.open_service` — open a watched service in the browser.
   *
   * Two row ids. The port comes from the workspace's own row and the address is
   * built in Rust; a service that is not running is refused rather than opened.
   */
  workspacesOpenService: (workspaceId: number, serviceId: number): Promise<Launched> =>
    call('workspaces_open_service', { workspaceId, serviceId }),

  /**
   * `workspaces.actions` — what this workspace can be asked to do.
   *
   * Resolved against this machine on every call, so an action whose application
   * has been uninstalled is a sentence on the row rather than an error after a
   * click.
   */
  workspacesActions: (workspaceId: number): Promise<WorkspaceAction[]> =>
    call('workspaces_actions', { workspaceId }),

  /**
   * `workspaces.action_catalogue` — every action Mira has, with the ones this
   * workspace already uses marked.
   *
   * A list built from an array compiled into the binary. There is no way to
   * write one, which is why a choice can travel back as an identity.
   */
  workspacesActionCatalogue: (workspaceId: number): Promise<ActionOffer[]> =>
    call('workspaces_action_catalogue', { workspaceId }),

  /**
   * `workspaces.set_action` — give this workspace an action, or take it away.
   *
   * The identity, never a command. An id naming no catalogue row is refused
   * before it is stored.
   */
  workspacesSetAction: (
    workspaceId: number,
    action: ActionId,
    wanted: boolean,
  ): Promise<WorkspaceAction[]> =>
    call('workspaces_set_action', { workspaceId, action, wanted }),

  /**
   * `workspaces.perform_action` — do one of this workspace's actions.
   *
   * Two identities in, a sentence out. Which directory, which application,
   * which address and which service are all resolved in Rust from Mira's own
   * rows and its own compiled tables.
   */
  workspacesPerformAction: (workspaceId: number, action: ActionId): Promise<Performed> =>
    call('workspaces_perform_action', { workspaceId, action }),
};
