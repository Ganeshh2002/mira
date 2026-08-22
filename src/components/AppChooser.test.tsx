import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Catalogue } from '../bindings/Catalogue';
import type { ChosenApp } from '../bindings/ChosenApp';
import { AppChooser } from './AppChooser';
import { renderApp } from '../test/render';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const CATALOGUE: Catalogue = {
  kind: 'editor',
  automatic: 'Visual Studio Code',
  options: [
    { id: 'vscode', name: 'Visual Studio Code', installed: true, openable: true },
    { id: 'zed', name: 'Zed', installed: true, openable: true },
    { id: 'nova', name: 'Nova', installed: false, openable: true },
    { id: 'neovim', name: 'Neovim', installed: true, openable: false },
  ],
};

/** Serve a catalogue and a resolved choice, and remember what was preferred. */
function backend(chosen: ChosenApp, catalogue: Catalogue = CATALOGUE) {
  invoke.mockImplementation((command: string) => {
    if (command === 'workspaces_catalogue') return Promise.resolve(catalogue);
    if (command === 'workspaces_chosen') return Promise.resolve(chosen);
    if (command === 'workspaces_prefer') {
      return Promise.resolve({ id: 7, projectId: 1, preferences: [] });
    }
    return Promise.resolve(undefined);
  });
}

function show(preferred: string | null = null) {
  return renderApp(<AppChooser workspaceId={7} kind="editor" preferred={preferred} />);
}

/** Open the chooser. */
async function open() {
  await userEvent.click(await screen.findByRole('button', { name: /Editor application/ }));
  return screen.getByRole('menu', { name: 'Editor application' });
}

/** What the last `workspaces.prefer` was told. */
function lastChoice() {
  const calls = invoke.mock.calls.filter(([command]) => command === 'workspaces_prefer');
  return calls[calls.length - 1]?.[1] as
    { workspaceId: number; kind: string; application: string | null } | undefined;
}

beforeEach(() => {
  invoke.mockReset();
});

describe('choosing an application', () => {
  it('offers only what Mira said it looks for', async () => {
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    const menu = await open();
    const items = within(menu).getAllByRole('menuitem');

    expect(items.map((item) => item.textContent)).toEqual([
      'Automatic · Visual Studio Codechosen',
      'Visual Studio Codeinstalled',
      'Zedinstalled',
      'Novanot installed',
      'Neovimcannot open a folder',
    ]);
  });

  it('sends back the id it was given, and nothing else', async () => {
    // The contract the slice rests on: a choice is an identity from a list Mira
    // produced. Nothing here builds one, and nothing here is a path.
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    await open();
    await userEvent.click(screen.getByRole('menuitem', { name: /Zed/ }));

    await waitFor(() =>
      expect(lastChoice()).toEqual({ workspaceId: 7, kind: 'editor', application: 'zed' }),
    );

    const sent = JSON.stringify(lastChoice());
    expect(sent).not.toContain('/');
    expect(sent).not.toContain('Zed.app');
    expect(sent).not.toContain('Applications');
  });

  it('says what automatic does today, rather than leaving it a mystery', async () => {
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    const menu = await open();
    expect(
      within(menu).getByRole('menuitem', { name: /Automatic · Visual Studio Code/ }),
    ).toBeInTheDocument();
  });

  it('goes back to automatic by choosing it, not by clearing a field', async () => {
    backend({ state: 'ready', id: 'zed', name: 'Zed' });
    show('zed');

    await open();
    await userEvent.click(screen.getByRole('menuitem', { name: /Automatic/ }));

    await waitFor(() => expect(lastChoice()?.application).toBeNull());
  });

  it('shows the choice on the control once one is made', async () => {
    backend({ state: 'ready', id: 'zed', name: 'Zed' });
    show('zed');

    expect(
      await screen.findByRole('button', { name: /Editor application.*Zed/ }),
    ).toBeInTheDocument();
  });

  it('says which rows are installed in words, not only by shade', async () => {
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    const menu = await open();
    expect(
      within(menu).getByRole('menuitem', { name: /Nova.*not installed/ }),
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: /Neovim.*cannot open a folder/ }),
    ).toBeInTheDocument();
  });

  it('has no field to type an application into', async () => {
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    await open();

    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  });

  it('closes with Escape and gives the focus back', async () => {
    backend({ state: 'automatic', application: 'Visual Studio Code' });
    show();

    const trigger = await screen.findByRole('button', { name: /Editor application/ });
    await userEvent.click(trigger);
    await userEvent.keyboard('{Escape}');

    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });
});

describe('a choice that stopped being true', () => {
  it('names the application that went missing, and says nothing else will open', async () => {
    // The honest unavailable state. Silently opening VS Code instead would mean
    // the person never finds out Zed is gone.
    backend({ state: 'missing', id: 'zed', name: 'Zed' });
    show('zed');

    expect(
      await screen.findByText(
        'Zed is not on this machine. Nothing else will be opened — choose another.',
      ),
    ).toBeInTheDocument();
  });

  it('leaves the menu right there to choose another', async () => {
    backend({ state: 'missing', id: 'zed', name: 'Zed' });
    show('zed');

    await open();
    await userEvent.click(
      screen.getByRole('menuitem', { name: /^Visual Studio Code installed$/ }),
    );

    await waitFor(() => expect(lastChoice()?.application).toBe('vscode'));
  });

  it('says an application that cannot open a folder is a different problem', async () => {
    backend({ state: 'notOpenable', id: 'neovim', name: 'Neovim' });
    show('neovim');

    expect(
      await screen.findByText(/Neovim is here, and is not something Mira can open a folder in/),
    ).toBeInTheDocument();
  });

  it('says when a choice came from a platform this one knows nothing about', async () => {
    backend({ state: 'unknown', id: 'nova' });
    show('nova');

    expect(
      await screen.findByText(/uses nova, which Mira does not look for on this platform/),
    ).toBeInTheDocument();
  });

  it('says nothing at all when the choice is working', async () => {
    backend({ state: 'ready', id: 'zed', name: 'Zed' });
    show('zed');

    await screen.findByRole('button', { name: /Editor application.*Zed/ });
    expect(screen.queryByText(/not on this machine/)).not.toBeInTheDocument();
    expect(screen.queryByText(/cannot open/)).not.toBeInTheDocument();
  });

  it('shows the reason a choice was refused', async () => {
    backend({ state: 'automatic', application: null });
    invoke.mockImplementation((command: string) => {
      if (command === 'workspaces_catalogue') return Promise.resolve(CATALOGUE);
      if (command === 'workspaces_chosen') {
        return Promise.resolve({ state: 'automatic', application: null });
      }
      return Promise.reject(
        new Error('Mira has no application called something-else on this platform.'),
      );
    });
    show();

    await open();
    await userEvent.click(screen.getByRole('menuitem', { name: /Zed/ }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Mira has no application called something-else on this platform.',
    );
  });
});

describe('a platform that looks for nothing', () => {
  it('says so rather than showing an empty menu', async () => {
    backend(
      { state: 'automatic', application: null },
      { kind: 'editor', automatic: null, options: [] },
    );
    show();

    const menu = await open();
    expect(
      within(menu).getByText('Mira does not look for any editor on this platform.'),
    ).toBeInTheDocument();
  });

  it('says automatic has nothing to pick when the machine has none of them', async () => {
    backend(
      { state: 'automatic', application: null },
      {
        ...CATALOGUE,
        automatic: null,
        options: CATALOGUE.options.map((o) => ({ ...o, installed: false })),
      },
    );
    show();

    const menu = await open();
    expect(
      within(menu).getByRole('menuitem', { name: /Automatic · nothing here yet/ }),
    ).toBeInTheDocument();
  });
});

describe('one workspace at a time', () => {
  it('asks and answers for its own workspace only', async () => {
    // Two workspaces side by side, each with its own choice. Nothing here is
    // keyed by kind alone, which is what would make one workspace's choice leak
    // into the other's row.
    invoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === 'workspaces_catalogue') return Promise.resolve(CATALOGUE);
      if (command === 'workspaces_chosen') {
        return Promise.resolve(
          args?.['workspaceId'] === 7
            ? { state: 'ready', id: 'zed', name: 'Zed' }
            : { state: 'ready', id: 'vscode', name: 'Visual Studio Code' },
        );
      }
      return Promise.resolve({ id: 7, projectId: 1, preferences: [] });
    });

    renderApp(
      <>
        <AppChooser workspaceId={7} kind="editor" preferred="zed" />
        <AppChooser workspaceId={8} kind="editor" preferred="vscode" />
      </>,
    );

    expect(
      await screen.findByRole('button', { name: /Editor application.*Zed/ }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('button', { name: /Editor application.*Visual Studio Code/ }),
    ).toBeInTheDocument();

    const asked = invoke.mock.calls
      .filter(([command]) => command === 'workspaces_chosen')
      .map(([, args]) => (args as { workspaceId: number }).workspaceId);

    expect(new Set(asked)).toEqual(new Set([7, 8]));
  });
});
