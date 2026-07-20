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

export function useImportProject() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.projectImport({ path })),
    onSuccess: () => {
      client.invalidateQueries({ queryKey: qk.projects });
    },
  });
}
