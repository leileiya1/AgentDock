import { useState } from "react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { PathField } from "@/components/PathField";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";
import { Button } from "@/components/ui/button";
import { useEnv, useSetCliPath } from "@/hooks/useEnv";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { sectionCls, sectionH } from "@/routes/Settings";
import { cn } from "@/lib/utils";

/** Git is a runtime prerequisite, so keep it separate from selectable AI Providers. */
export function EnvSection() {
  const env = useEnv();
  const setPath = useSetCliPath();
  const [details, setDetails] = useState(false);
  const [path, setPathValue] = useState("");

  const save = async () => {
    try {
      await setPath.mutateAsync({ tool: "git", path });
      toast.info("Git 路径已保存");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <section className={sectionCls}>
      <div className="mb-3 flex items-center justify-between">
        <h2 className={cn(sectionH, "mb-0")}>基础环境</h2>
        <Button variant="outline" size="sm" onClick={() => env.refetch()} disabled={env.isFetching}>
          {env.isFetching ? "检测中…" : "重新检测"}
        </Button>
      </div>
      {env.isLoading ? (
        <SkeletonRows rows={1} />
      ) : env.isError ? (
        <ErrorState error={env.error} onRetry={() => env.refetch()} compact />
      ) : env.data && (
        <div className="rounded-[var(--radius-control)] border border-line bg-app px-3 py-2.5">
          <div className="flex items-center gap-3">
            <ProviderIcon provider="git" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 font-medium">
                Git
                <span className={cn("size-2 rounded-full", env.data.git.compatible ? "bg-ok" : "bg-bad")} />
              </div>
              <div className={cn("text-[12px]", env.data.git.compatible ? "text-t3" : "text-bad")}>
                {env.data.git.compatible ? "已就绪" : "需要处理"}
              </div>
            </div>
            <Button variant="ghost" size="sm" onClick={() => {
              setPathValue(env.data?.git.path ?? "");
              setDetails((value) => !value);
            }}>{details ? "收起" : "详情"}</Button>
          </div>
          {details && (
            <div className="ml-[52px] mt-2 rounded-md bg-panel/70 px-3 py-2 text-[12px] text-t3">
              {env.data.git.version && <div className="mb-2">版本 {env.data.git.version}</div>}
              <PathField value={path} onChange={setPathValue} onDetect={save} detecting={setPath.isPending} />
            </div>
          )}
        </div>
      )}
    </section>
  );
}
