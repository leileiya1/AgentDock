import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import type { ApiCredentialArgs, CliCredentialArgs, CliInstallArgs, CliPathArgs } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

export function useEnv() {
  return useQuery({
    queryKey: qk.env,
    queryFn: () => unwrap(commands.envCheck()),
  });
}

export function useOnboarding() {
  return useQuery({
    queryKey: qk.onboarding,
    queryFn: () => unwrap(commands.onboardingCheck()),
  });
}

export function useSetCliPath() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: CliPathArgs) => unwrap(commands.envSetCliPath(args)),
    onSuccess: (env) => {
      client.setQueryData(qk.env, env);
      client.invalidateQueries({ queryKey: qk.onboarding });
    },
  });
}

function useEnvMutation<T>(mutationFn: (args: T) => ReturnType<typeof commands.envCheck>) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (args: T) => unwrap(mutationFn(args)),
    onSuccess: (env) => {
      client.setQueryData(qk.env, env);
      client.invalidateQueries({ queryKey: qk.onboarding });
      client.invalidateQueries({ queryKey: qk.providers });
    },
  });
}

export function useInstallCli() {
  return useEnvMutation<CliInstallArgs>((args) => commands.cliInstall(args));
}

export function useSetCliCredential() {
  return useEnvMutation<CliCredentialArgs>((args) => commands.cliCredentialSet(args));
}

export function useDeleteCliCredential() {
  return useEnvMutation<CliCredentialArgs>((args) => commands.cliCredentialDelete(args));
}

export function useSetApiCredential() {
  return useEnvMutation<ApiCredentialArgs>((args) => commands.apiCredentialSet(args));
}

export function useDeleteApiCredential() {
  return useEnvMutation<ApiCredentialArgs>((args) => commands.apiCredentialDelete(args));
}

export function useCompleteOnboarding() {
  return useMutation({
    mutationFn: () => unwrap(commands.onboardingComplete()),
  });
}
