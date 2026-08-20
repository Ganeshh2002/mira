import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App, type Surface } from './App';
import './styles/app.css';

/**
 * Which surface this window is. Mira uses an in-memory model rather than a URL
 * router: there are five surfaces and no address bar (architecture.md §7).
 */
const surface: Surface = getCurrentWindow().label === 'settings' ? 'settings' : 'main';

const client = new QueryClient({
  defaultOptions: {
    queries: {
      // Nothing polls in Slice 0. Refetching is driven by explicit invalidation
      // and, from Slice 1, by backend events — never by a timer started here.
      refetchOnWindowFocus: false,
      staleTime: Infinity,
      retry: false,
    },
  },
});

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
