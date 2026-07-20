import { useQuery } from "@tanstack/react-query";
import { commands } from "@/generated/bindings";
import { unwrap } from "@/lib/commands";
import { qk } from "@/lib/queryKeys";

/** Runtime catalog: includes built-ins and protocol sidecars discovered by the Rust core. */
export function useProviders() {
  return useQuery({
    queryKey: qk.providers,
    queryFn: () => unwrap(commands.providerList()),
    staleTime: 30_000,
  });
}
