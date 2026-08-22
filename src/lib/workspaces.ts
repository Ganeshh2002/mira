/**
 * Query keys for workspaces.
 *
 * Kept in one place so the detail view and the list agree on what to invalidate:
 * a workspace changing is a change to *its project's* list, not to a global one.
 */
export const workspaceKeys = {
  of: (projectId: number) => ['workspaces', projectId] as const,
  applications: ['workspaces', 'applications'] as const,
  openable: ['workspaces', 'openable'] as const,
  catalogue: (kind: string) => ['workspaces', 'catalogue', kind] as const,
  chosen: (workspaceId: number) => ['workspaces', 'chosen', workspaceId] as const,
  services: (workspaceId: number) => ['workspaces', workspaceId, 'services'] as const,
  offers: (workspaceId: number) => ['workspaces', workspaceId, 'offers'] as const,
};
