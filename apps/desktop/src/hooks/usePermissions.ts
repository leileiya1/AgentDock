import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type PermissionDecisionInput,
  type PermissionRule,
} from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/**
 * 权限代理数据接入 (06 §9). 只调用已生成的真实 Rust/Tauri 绑定，绝不在前端模拟批准：
 * - permission_request_list / permission_decide / permission_rule_list / permission_rule_revoke。
 * 请求列表由 task:changed 事件驱动刷新 (见 useGlobalEvents)，UI 关闭重开后仍从后端读取
 * 未处理请求与剩余有效期 (§9 第 16 条)。
 */

export function usePermissionRequests(taskId: string | undefined, enabled = true) {
  return useQuery({
    queryKey: qk.permissionRequests(taskId ?? "none"),
    queryFn: () => unwrap(commands.permissionRequestList({ taskId: taskId! })),
    enabled: !!taskId && enabled,
    // 剩余有效期是敏感的——保持较短的过期时间，让重新聚焦时拿到最新状态。
    staleTime: 5_000,
    refetchOnWindowFocus: true,
  });
}

export function usePermissionRules(projectId: string | undefined, enabled = true) {
  return useQuery({
    queryKey: qk.permissionRules(projectId ?? "none"),
    queryFn: () => unwrap(commands.permissionRuleList({ projectId: projectId! })),
    enabled: !!projectId && enabled,
  });
}

/**
 * 提交一个真实的权限决定。决定成功后：
 * - 刷新该任务的请求列表（状态从 pending 变为 approved/denied/cancelled）；
 * - project_rule 授权会新建规则 → 刷新项目规则列表；
 * - 任务本身可能从 BLOCKED 恢复 → 刷新任务与事件缓存。
 */
export function usePermissionDecide(taskId: string | undefined, projectId: string | undefined) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (input: PermissionDecisionInput) => unwrap(commands.permissionDecide({ input })),
    onSuccess: () => {
      if (taskId) {
        client.invalidateQueries({ queryKey: qk.permissionRequests(taskId) });
        client.invalidateQueries({ queryKey: qk.task(taskId) });
        client.invalidateQueries({ queryKey: qk.events(taskId) });
        client.invalidateQueries({ queryKey: qk.queue(taskId) });
      }
      if (projectId) client.invalidateQueries({ queryKey: qk.permissionRules(projectId) });
    },
  });
}

/** 撤销一条项目规则 (§9 第 19 条)。撤销只阻止下一次执行，不影响已完成动作。 */
export function useRevokeRule(projectId: string | undefined) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (ruleId: string) =>
      unwrap(commands.permissionRuleRevoke({ projectId: projectId!, ruleId })),
    onSuccess: (rule) => {
      client.setQueryData<PermissionRule[]>(qk.permissionRules(rule.projectId), (prev) =>
        prev?.map((r) => (r.id === rule.id ? rule : r))
      );
    },
  });
}
