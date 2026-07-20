import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type BudgetLimitPatch, type ExecutionNode, type RollbackStrategy, type TaskDetail } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";
import { syncTaskCaches } from "@/hooks/useTasks";

export function useGovernance(taskId: string | undefined, revision?: number) {
  return useQuery({
    queryKey: qk.governance(taskId ?? "none", revision ?? -1),
    queryFn: () => unwrap(commands.taskGovernanceGet({ taskId: taskId!, revision: revision ?? null })),
    enabled: !!taskId,
  });
}

function useTaskGovernanceMutation<A>(fn: (args: A) => Promise<TaskDetail>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (detail) => {
      syncTaskCaches(client, detail);
      client.invalidateQueries({ queryKey: qk.governance(detail.id, detail.currentRevision) });
    },
  });
}

export const usePlanApprove = () =>
  useTaskGovernanceMutation((args: { taskId: string; planId: string }) =>
    unwrap(commands.taskPlanApprove(args))
  );

export const usePlanReject = () =>
  useTaskGovernanceMutation((args: { taskId: string; planId: string; reason: string }) =>
    unwrap(commands.taskPlanReject(args))
  );

export const useBudgetUpdate = () =>
  useTaskGovernanceMutation((args: { taskId: string; limits: BudgetLimitPatch }) =>
    unwrap(commands.taskBudgetUpdate(args))
  );

export function useQualityReplay() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: { taskId: string; revision?: number }) =>
      unwrap(commands.taskQualityReplay({ taskId: args.taskId, revision: args.revision ?? null })),
    onSuccess: (quality) => {
      client.invalidateQueries({ queryKey: qk.governance(quality.taskId, quality.revision) });
    },
  });
}

export const useDeliveryStart = () =>
  useTaskGovernanceMutation((taskId: string) => unwrap(commands.taskDeliveryStart({ taskId })));

export const useDeliveryRefresh = () =>
  useTaskGovernanceMutation((taskId: string) => unwrap(commands.taskDeliveryRefresh({ taskId })));

export const useRollback = () =>
  useTaskGovernanceMutation((args: { taskId: string; strategy: RollbackStrategy }) =>
    unwrap(commands.taskRollback(args))
  );

export function useExecutionNodes() {
  return useQuery({ queryKey: qk.executionNodes, queryFn: () => unwrap(commands.executionNodeList()) });
}

export function useExecutionNodeMutations() {
  const client = useQueryClient();
  const refresh = () => client.invalidateQueries({ queryKey: qk.executionNodes });
  return {
    upsert: useMutation({ mutationFn: (node: ExecutionNode) => unwrap(commands.executionNodeUpsert({ node })), onSuccess: refresh }),
    check: useMutation({ mutationFn: (nodeId: string) => unwrap(commands.executionNodeCheck({ nodeId })), onSuccess: refresh }),
    remove: useMutation({ mutationFn: (nodeId: string) => unwrap(commands.executionNodeDelete({ nodeId })), onSuccess: refresh }),
  };
}
