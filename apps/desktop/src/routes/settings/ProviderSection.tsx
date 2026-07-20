import { RefreshCw } from "lucide-react";
import { useEnv } from "@/hooks/useEnv";
import { useProviders } from "@/hooks/useProviders";
import { ProviderCatalog } from "@/components/ProviderCatalog";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { sectionCls, sectionH } from "@/routes/Settings";

export function ProviderSection() {
  const env = useEnv();
  const providers = useProviders();
  const loading = env.isLoading || providers.isLoading;
  const refresh = () => {
    env.refetch();
    providers.refetch();
  };

  return (
    <section className={sectionCls}>
      <div className="mb-3 flex items-center justify-between">
        <div>
          <h2 className={sectionH + " !mb-0"}>AI Provider</h2>
          <p className="mt-0.5 text-[12px] text-t3">常用连接优先展示，其他适配器收进“更多”。</p>
        </div>
        <Button variant="outline" size="sm" onClick={refresh} disabled={env.isFetching || providers.isFetching}>
          <RefreshCw className={`size-3.5 ${env.isFetching || providers.isFetching ? "animate-spin" : ""}`} />
          刷新
        </Button>
      </div>
      {loading ? (
        <SkeletonRows rows={5} />
      ) : env.isError ? (
        <ErrorState error={env.error} onRetry={refresh} compact />
      ) : providers.isError ? (
        <ErrorState error={providers.error} onRetry={refresh} compact />
      ) : env.data ? (
        <div className="flex flex-col gap-4">
          <ProviderCatalog env={env.data} providers={providers.data} />
        </div>
      ) : null}
    </section>
  );
}
