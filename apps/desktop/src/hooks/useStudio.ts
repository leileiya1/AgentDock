import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentKind, type AgentProfilePatch, type ProjectAppearancePatch, type ProjectMemberInput } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/** 每个项目的外观（类型、屋顶颜色、门牌样式、院子摆件），没存过的是空值。 */
export function useProjectAppearance() {
  return useQuery({
    queryKey: qk.projectAppearance,
    queryFn: () => unwrap(commands.projectAppearanceList()),
    staleTime: 60_000,
  });
}

/** 改项目外观：字段不传 = 不改，传 null = 清空。 */
export function useUpdateProjectAppearance() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: { projectId: string; patch: ProjectAppearancePatch }) => unwrap(commands.projectAppearanceUpdate(args)),
    onSuccess: () => client.invalidateQueries({ queryKey: qk.projectAppearance }),
  });
}

/** 项目分配的 Agent，按摆放顺序。 */
export function useProjectMembers(projectId: string | undefined) {
  return useQuery({
    queryKey: qk.projectMembers(projectId ?? ""),
    queryFn: () => unwrap(commands.projectMembersGet({ projectId: projectId! })),
    enabled: !!projectId,
  });
}

/** 整组替换项目成员；数组顺序就是小镇里的摆放顺序。 */
export function useSetProjectMembers() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: { projectId: string; members: ProjectMemberInput[] }) => unwrap(commands.projectMembersSet(args)),
    onSuccess: (_data, { projectId }) => {
      client.invalidateQueries({ queryKey: qk.projectMembers(projectId) });
      client.invalidateQueries({ queryKey: qk.projectSummaries });
    },
  });
}

/** 用户自定义过的 Agent 档案（名字、配色、配件、口头禅），全局共用。 */
export function useAgentProfiles() {
  return useQuery({
    queryKey: qk.agentProfiles,
    queryFn: () => unwrap(commands.agentProfilesList()),
    staleTime: 60_000,
  });
}

export function useUpdateAgentProfile() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: { agent: AgentKind; patch: AgentProfilePatch }) => unwrap(commands.agentProfileUpdate(args)),
    onSuccess: () => client.invalidateQueries({ queryKey: qk.agentProfiles }),
  });
}

/** 恢复默认外观（删除档案）。 */
export function useResetAgentProfile() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (agent: AgentKind) => unwrap(commands.agentProfileReset({ agent })),
    onSuccess: () => client.invalidateQueries({ queryKey: qk.agentProfiles }),
  });
}
