import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import type { TaskCleanupArgs } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

export function useStorageReport() {
  return useQuery({
    queryKey: qk.storage,
    queryFn: () => unwrap(commands.storageReport()),
  });
}

export function useTrash() {
  return useQuery({
    queryKey: qk.trash,
    queryFn: () => unwrap(commands.trashList()),
  });
}

export function useDatabaseBackups() {
  return useQuery({
    queryKey: qk.databaseBackups,
    queryFn: () => unwrap(commands.databaseBackupList()),
  });
}

function useStorageMutation<A, R>(fn: (args: A) => Promise<R>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: () => {
      client.invalidateQueries({ queryKey: qk.storage });
      client.invalidateQueries({ queryKey: qk.trash });
      client.invalidateQueries({ queryKey: qk.databaseBackups });
    },
  });
}

export const useStorageCleanup = () =>
  useStorageMutation<void, unknown>(() => unwrap(commands.storageCleanup()));

export const useTaskCleanup = () =>
  useStorageMutation((args: TaskCleanupArgs) => unwrap(commands.taskCleanup(args)));

export const useTaskRestore = () =>
  useStorageMutation((taskId: string) => unwrap(commands.taskRestore({ taskId })));

export const useTrashEmpty = () =>
  useStorageMutation<void, unknown>(() => unwrap(commands.trashEmpty()));

export const useDatabaseBackupCreate = () =>
  useStorageMutation<void, unknown>(() => unwrap(commands.databaseBackupCreate()));

export const useDatabaseBackupRestore = () =>
  useStorageMutation((path: string) => unwrap(commands.databaseBackupRestore({ path })));
