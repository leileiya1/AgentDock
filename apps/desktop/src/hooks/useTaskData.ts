import { useQuery } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

export function useDiff(taskId: string | undefined, revision: number | undefined) {
  return useQuery({
    queryKey: qk.diff(taskId ?? "none", revision ?? -1),
    queryFn: () => unwrap(commands.diffGet({ taskId: taskId!, revision: revision! })),
    enabled: !!taskId && revision != null && revision > 0,
    staleTime: 60_000,
  });
}

export function useReview(taskId: string | undefined, revision: number | undefined) {
  return useQuery({
    queryKey: qk.review(taskId ?? "none", revision ?? -1),
    queryFn: () => unwrap(commands.reviewGet({ taskId: taskId!, revision: revision! })),
    enabled: !!taskId && revision != null && revision > 0,
    staleTime: 60_000,
  });
}

export function useRuns(taskId: string | undefined) {
  return useQuery({
    queryKey: qk.runs(taskId ?? "none"),
    queryFn: () => unwrap(commands.runList({ taskId: taskId! })),
    enabled: !!taskId,
  });
}

export function useEvents(taskId: string | undefined) {
  return useQuery({
    queryKey: qk.events(taskId ?? "none"),
    queryFn: () => unwrap(commands.eventsList({ taskId: taskId!, afterId: null, limit: null })),
    enabled: !!taskId,
  });
}
