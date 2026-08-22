import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { PortRow } from '../bindings/PortRow';
import type { PortsView } from '../bindings/PortsView';
import type { Project } from '../bindings/Project';
import { App } from '../App';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

const NOW = 1_800_000_000;

const aviora: Project = {
  id: 1,
  name: 'aviora',
  rootPath: '/home/dev/aviora',
  isGit: true,
  gitRoot: null,
  markers: ['node'],
  lastOpenedAt: NOW,
  createdAt: NOW,
  updatedAt: NOW,
};

function row(port: number, at: number, overrides: Partial<PortRow> = {}): PortRow {
  return {
    at,
    port,
    address: '127.0.0.1',
    process: {
      pid: 18_234,
      name: 'node',
      executable: '/usr/local/bin/node',
      parent: 1,
      workingDirectory: '/home/dev/aviora',
      cpuShare: 12.5,
      memoryBytes: 188_743_680,
      uptimeSeconds: 7_200,
    },
    ...overrides,
  };
}

function view(overrides: Partial<PortsView> = {}): PortsView {
  return {
    projects: [{ projectId: 1, project: 'aviora', rows: [row(3_000, 0)] }],
    unattributed: [],
    observedAt: NOW,
    error: null,
    ...overrides,
  };
}

function backend(ports: PortsView = view()) {
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case 'projects_list':
        return Promise.resolve([aviora]);
      case 'live_ports':
        return Promise.resolve(ports);
      case 'workspaces_list':
      case 'workspaces_applications':
      case 'workspaces_openable':
        return Promise.resolve([]);
      case 'live_refresh':
      case 'live_snapshot':
        return Promise.resolve({
          projects: [],
          services: { services: [], error: null, observedAt: NOW },
        });
      default:
        return Promise.resolve(null);
    }
  });
}

/** Open the machine-wide Ports view from the project nav. */
async function openPorts() {
  renderApp(<App surface="main" />);
  await userEvent.click(await screen.findByRole('button', { name: /^Ports/ }));
  return screen.findByRole('heading', { name: /ports/i });
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.setSystemTime(new Date(NOW * 1000));
});

describe('the machine-wide Ports view', () => {
  it('is reachable from the nav and says it is about this machine', async () => {
    backend();
    await openPorts();

    // The label that keeps it from reading as a project.
    expect(screen.getByText(/this machine/i)).toBeInTheDocument();
    expect(screen.getByText(/everything listening on this computer/i)).toBeInTheDocument();
  });

  it('says plainly that it is not what a workspace watches', async () => {
    backend();
    await openPorts();

    expect(screen.getByText(/not what a workspace watches/i)).toBeInTheDocument();
  });

  it('groups listeners under the project that owns them', async () => {
    backend(
      view({
        projects: [
          { projectId: 1, project: 'aviora', rows: [row(3_000, 0)] },
          { projectId: 2, project: 'mira', rows: [row(8_080, 1)] },
        ],
      }),
    );
    await openPorts();

    const ours = await screen.findByRole('list', { name: 'aviora' });
    expect(within(ours).getByText(':3000')).toBeInTheDocument();

    const theirs = screen.getByRole('list', { name: 'mira' });
    expect(within(theirs).getByText(':8080')).toBeInTheDocument();
    expect(within(theirs).queryByText(':3000')).not.toBeInTheDocument();
  });

  it('shows what it could not place, with the reason, rather than dropping it', async () => {
    backend(
      view({
        projects: [],
        unattributed: [row(9_999, 0, { process: null })],
      }),
    );
    await openPorts();

    const unplaced = await screen.findByRole('list', { name: /not in any project/i });
    expect(within(unplaced).getByText(':9999')).toBeInTheDocument();
    expect(screen.getByText(/could not place these/i)).toBeInTheDocument();
  });

  it('says nothing is listening rather than showing an empty box', async () => {
    backend(view({ projects: [], unattributed: [] }));
    await openPorts();

    expect(await screen.findByText(/nothing is listening/i)).toBeInTheDocument();
  });

  it('says it has not looked, which is not the same as nothing listening', async () => {
    backend(view({ projects: [], unattributed: [], observedAt: null }));
    await openPorts();

    expect(await screen.findByText(/has not read the list/i)).toBeInTheDocument();
    expect(screen.queryByText(/nothing is listening/i)).not.toBeInTheDocument();
  });

  it('passes the platform reason through when the read failed', async () => {
    backend(
      view({
        projects: [],
        unattributed: [],
        error: 'Reading the list of listening ports was refused.',
      }),
    );
    await openPorts();

    expect(await screen.findByText(/port information unavailable/i)).toBeInTheDocument();
    expect(screen.getByText(/was refused/i)).toBeInTheDocument();
  });

  it('opens a listener by its position, never by its port', async () => {
    backend(view({ projects: [{ projectId: 1, project: 'aviora', rows: [row(3_000, 7)] }] }));
    await openPorts();

    await userEvent.click(await screen.findByRole('button', { name: /open port 3000/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('live_open_service', { at: 7 }));
    const sent = invoke.mock.calls.find(([name]) => name === 'live_open_service')?.[1];
    expect(sent).toEqual({ at: 7 });
    expect(JSON.stringify(sent)).not.toMatch(/3000|http|localhost|18234/);
  });

  it('offers nothing that could stop a process', async () => {
    backend();
    await openPorts();

    await screen.findByRole('list', { name: 'aviora' });
    for (const destructive of [/kill/i, /terminate/i, /^stop$/i, /^restart$/i, /force/i]) {
      expect(screen.queryByRole('button', { name: destructive })).not.toBeInTheDocument();
    }
  });
});

describe('process detail on a row', () => {
  it('shows cpu, memory and uptime with their labels', async () => {
    backend();
    await openPorts();

    const list = await screen.findByRole('list', { name: 'aviora' });
    expect(within(list).getByText('CPU')).toBeInTheDocument();
    // Whole numbers at or above ten, because a tenth of a percent of a CPU is
    // noise a person cannot act on.
    expect(within(list).getByText('13%')).toBeInTheDocument();
    expect(within(list).getByText('Memory')).toBeInTheDocument();
    expect(within(list).getByText('180 MB')).toBeInTheDocument();
    expect(within(list).getByText('Running for')).toBeInTheDocument();
    expect(within(list).getByText('2h')).toBeInTheDocument();
  });

  it('keeps a decimal below ten percent, where the difference is readable', async () => {
    backend(
      view({
        projects: [
          {
            projectId: 1,
            project: 'aviora',
            rows: [row(3_000, 0, { process: { ...row(3_000, 0).process!, cpuShare: 4.2 } })],
          },
        ],
      }),
    );
    await openPorts();

    const list = await screen.findByRole('list', { name: 'aviora' });
    expect(within(list).getByText('4.2%')).toBeInTheDocument();
  });

  it('says a share was not measured rather than calling the process idle', async () => {
    // The first reading has nothing to compare against. "0%" would be a claim
    // Mira has not established.
    backend(
      view({
        projects: [
          {
            projectId: 1,
            project: 'aviora',
            rows: [row(3_000, 0, { process: { ...row(3_000, 0).process!, cpuShare: null } })],
          },
        ],
      }),
    );
    await openPorts();

    const list = await screen.findByRole('list', { name: 'aviora' });
    expect(within(list).getByText('not measured yet')).toBeInTheDocument();
    expect(within(list).queryByText('0%')).not.toBeInTheDocument();
  });

  it('distinguishes a genuinely idle process from an unmeasured one', async () => {
    backend(
      view({
        projects: [
          {
            projectId: 1,
            project: 'aviora',
            rows: [row(3_000, 0, { process: { ...row(3_000, 0).process!, cpuShare: 0 } })],
          },
        ],
      }),
    );
    await openPorts();

    const list = await screen.findByRole('list', { name: 'aviora' });
    expect(within(list).getByText('0.0%')).toBeInTheDocument();
    expect(within(list).queryByText('not measured yet')).not.toBeInTheDocument();
  });

  it('says the platform did not name a process rather than showing blanks', async () => {
    backend(
      view({
        projects: [],
        unattributed: [row(9_999, 0, { process: null })],
      }),
    );
    await openPorts();

    expect(
      await screen.findByText(/did not say which process is listening/i),
    ).toBeInTheDocument();
  });

  it('never renders a command line', async () => {
    backend();
    const heading = await openPorts();

    // Nothing resembling argv reaches the document, because nothing carries it.
    const text = heading.ownerDocument.body.textContent ?? '';
    for (const argv of ['--password', '--inspect', 'node --', 'sh -c', 'DATABASE_URL']) {
      expect(text).not.toContain(argv);
    }
  });
});
