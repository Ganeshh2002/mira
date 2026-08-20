import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import type { ReactElement } from 'react';

import { queryOptions } from '../lib/query';

/**
 * Render inside a fresh client that uses the **product's** query policy, exactly.
 *
 * Nothing is relaxed for the tests' convenience. A shorter `gcTime` or a stale
 * time of zero would make every query refetch on remount, and a test written
 * against that would pass while the shipped application showed a permanently
 * stale panel. Isolation comes from building a new client per render instead.
 */
export function renderApp(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: queryOptions() });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}
