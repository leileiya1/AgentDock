import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import type { GlobalSettings, ProjectSettings } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

export function useSettings() {
  return useQuery({
    queryKey: qk.settings,
    queryFn: () => unwrap(commands.settingsGet()),
  });
}

export function useUpdateSettings() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (patch: GlobalSettings) => unwrap(commands.settingsUpdate({ patch })),
    onSuccess: (settings) => client.setQueryData(qk.settings, settings),
  });
}

export function useProjectSettings(projectId: string | undefined) {
  return useQuery({
    queryKey: qk.projectSettings(projectId ?? "none"),
    queryFn: () => unwrap(commands.projectSettingsGet({ projectId: projectId! })),
    enabled: !!projectId,
  });
}

export function useProjectGitCompatibility(projectId: string | undefined) {
  return useQuery({
    queryKey: qk.projectGitCompatibility(projectId ?? "none"),
    queryFn: () => unwrap(commands.projectGitCompatibility({ projectId: projectId! })),
    enabled: !!projectId,
  });
}

export function useUpdateProjectSettings(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (patch: ProjectSettings) =>
      unwrap(commands.projectSettingsUpdate({ projectId, patch })),
    onSuccess: (settings) => client.setQueryData(qk.projectSettings(projectId), settings),
  });
}

export function useProjectConfigTrust(projectId: string | undefined) {
  return useQuery({
    queryKey: qk.projectConfigTrust(projectId ?? "none"),
    queryFn: () => unwrap(commands.projectConfigTrustGet({ projectId: projectId! })),
    enabled: !!projectId,
  });
}

export function useApproveProjectConfig(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.projectConfigTrustApprove({ projectId })),
    onSuccess: (trust) => client.setQueryData(qk.projectConfigTrust(projectId), trust),
  });
}

export function useRevokeProjectConfig(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.projectConfigTrustRevoke({ projectId })),
    onSuccess: (trust) => client.setQueryData(qk.projectConfigTrust(projectId), trust),
  });
}
