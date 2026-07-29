import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import type { ProviderPreflightArgs, TaskPreflightReport } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/**
 * Probe whether a task's developer/reviewer selection can actually run before creating it. Runs a
 * live login + protocol probe (not just "command exists"), so "创建并立即开始" can refuse to spawn a
 * doomed run and list every unavailable Provider at once (P0-01/P0-02). Re-detecting also refreshes
 * the cached provider catalog so the rest of the UI reflects the same probe.
 */
export function useProviderPreflight() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: ProviderPreflightArgs): Promise<TaskPreflightReport> =>
      unwrap(commands.providerPreflight(args)),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: qk.providers });
      client.invalidateQueries({ queryKey: qk.env });
    },
  });
}
