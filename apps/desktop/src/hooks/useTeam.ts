import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type TeamAction, type TeamCreateInput, type TeamView } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";
export const teamKey = (id: string) => ["team", id] as const;
export const teamRecoveryKey = (id: string) => ["teamRecovery", id] as const;
export function useTeamRecovery(id: string, enabled: boolean) {
  return useQuery({ queryKey: teamRecoveryKey(id), enabled,
    queryFn: () => unwrap(commands.teamRecoveryGet({ taskId: id })),
    refetchInterval: enabled ? 3000 : false,
  });
}
export function useTeam(id: string | undefined) {
  return useQuery({ queryKey: teamKey(id ?? "none"), enabled: !!id,
    queryFn: () => unwrap(commands.teamGet({ taskId: id! })),
    refetchInterval: (query) => query.state.data && !["delivered", "cancelled"].includes(query.state.data.phase) ? 1500 : false,
  });
}
export function useTeamAction(id: string) {
  const client = useQueryClient();
  return useMutation({ mutationFn: (action: TeamAction) => unwrap(commands.teamAction({ taskId: id, action })),
    onSuccess: (view: TeamView) => { client.setQueryData(teamKey(id), view); client.invalidateQueries({ queryKey: teamRecoveryKey(id) }); client.invalidateQueries({ queryKey: qk.task(id) }); client.invalidateQueries({ queryKey: ["tasks"] }); },
  });
}
export function useCreateTeam() {
  const client = useQueryClient();
  return useMutation({ mutationFn: (input: TeamCreateInput) => unwrap(commands.teamCreate(input)),
    onSuccess: (view: TeamView) => { client.setQueryData(teamKey(view.taskId), view); client.invalidateQueries({ queryKey: ["tasks"] }); },
  });
}
