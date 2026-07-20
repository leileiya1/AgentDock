import { QueryClient } from "@tanstack/react-query";

/**
 * Invalidation is event-driven (02 §5): no window-focus refetch, no blanket
 * polling. The task-list bootstraps a 30s safety poll where it is used.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      refetchOnReconnect: false,
      retry: 1,
      staleTime: 15_000,
      gcTime: 5 * 60_000,
    },
    mutations: {
      retry: 0,
    },
  },
});
