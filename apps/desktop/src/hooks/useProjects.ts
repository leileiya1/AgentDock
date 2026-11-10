import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

export function useProjects() {
  return useQuery({
    queryKey: qk.projects,
    queryFn: () => unwrap(commands.projectList()),
  });
}

/**
 * 每个项目一行概况（小镇用）：任务数、在跑、等你、完结、近期测试失败、参与的 Agent。
 * 只读；任务状态会变，所以短间隔刷新，窗口不在前台时 React Query 自动暂停。
 */
export function useProjectSummaries() {
  return useQuery({
    queryKey: qk.projectSummaries,
    queryFn: () => unwrap(commands.projectSummaries()),
    refetchInterval: 15_000,
  });
}

export function useImportProject() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.projectImport({ path })),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: qk.projects });
      client.invalidateQueries({ queryKey: qk.projectSummaries });
      client.invalidateQueries({ queryKey: qk.projectAppearance });
    },
  });
}
