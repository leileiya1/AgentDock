import { useMutation } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";

export function useAuditExport(projectId: string) {
  return useMutation({
    mutationFn: (taskId: string | null) =>
      unwrap(commands.eventsExport({ projectId, taskId })),
  });
}
