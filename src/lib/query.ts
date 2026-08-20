import { QueryClient } from '@tanstack/react-query';

/**
 * The query policy, in one place.
 *
 * Both the application and the tests build their client from this, so a test can
 * never pass under settings the product does not ship. That mattered the moment
 * caching did: a suite with the default `staleTime` refetches everything and
 * would have hidden a stale Git panel entirely.
 *
 * **Nothing here polls.** There is no `refetchInterval` and there never will be
 * outside the scheduler (`architecture.md` §6); a guard test fails the build if
 * one appears. Reads happen when a person does something.
 */
export function queryOptions() {
  return {
    queries: {
      refetchOnWindowFocus: false,
      staleTime: Infinity,
      retry: false,
    },
  } as const;
}

export function createQueryClient(): QueryClient {
  return new QueryClient({ defaultOptions: queryOptions() });
}
