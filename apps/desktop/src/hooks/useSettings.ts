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

export function useUpdateProjectSettings(projectId: string) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (patch: ProjectSettings) =>
      unwrap(commands.projectSettingsUpdate({ projectId, patch })),
    onSuccess: (settings) => client.setQueryData(qk.projectSettings(projectId), settings),
  });
}
