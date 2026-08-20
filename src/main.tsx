import { QueryClientProvider } from '@tanstack/react-query';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App, type Surface } from './App';
import { createQueryClient } from './lib/query';
import './styles/app.css';

/**
 * Which surface this window is. Mira uses an in-memory model rather than a URL
 * router: there are five surfaces and no address bar (architecture.md §7).
 */
const surface: Surface = getCurrentWindow().label === 'settings' ? 'settings' : 'main';

const client = createQueryClient();

const root = document.getElementById('root');
if (!root) {
  throw new Error('Mira could not find its root element.');
}

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={client}>
      <App surface={surface} />
    </QueryClientProvider>
  </StrictMode>,
);
