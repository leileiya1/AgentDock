import { useState } from "react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { PathField } from "@/components/PathField";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";
import { Button } from "@/components/ui/button";
import { useEnv, useOnboarding, useSetCliPath } from "@/hooks/useEnv";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { sectionCls, sectionH } from "@/routes/Settings";
import { cn } from "@/lib/utils";

/** Git is a runtime prerequisite, so keep it separate from selectable AI Providers. */
export function EnvSection() {
  const env = useEnv();
  const onboarding = useOnboarding();
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
        <Button variant="outline" size="sm" onClick={() => { env.refetch(); onboarding.refetch(); }} disabled={env.isFetching || onboarding.isFetching}>
          {env.isFetching || onboarding.isFetching ? "检测中…" : "重新检测"}
        </Button>
      </div>
      {env.isLoading ? (
        <SkeletonRows rows={1} />
      ) : env.isError ? (
        <ErrorState error={env.error} onRetry={() => env.refetch()} compact />
      ) : env.data && (
        <div className="flex flex-col gap-3">
          <div className="grid gap-2 sm:grid-cols-2">
            <ReadinessCard
              title="应用基础环境"
              ready={onboarding.data?.appReady ?? false}
              detail={(onboarding.data?.appReady ?? false) ? "应用、Git 与本地数据目录可用" : "应用可打开，但基础依赖或磁盘需要处理"}
            />
            <ReadinessCard
              title="端到端工作流"
              ready={onboarding.data?.workflowReady ?? false}
              detail={(onboarding.data?.workflowReady ?? false) ? "至少一组开发 + 独立审查 Provider 可运行" : "尚无可完成计划 → 开发 → 审查的组合"}
            />
          </div>

          <div className="grid gap-x-4 gap-y-2 rounded-[var(--radius-control)] border border-line bg-app px-3 py-3 text-[12px] sm:grid-cols-2">
            <EnvFact label="操作系统" value={`${env.data.system.os}${env.data.system.osVersion ? ` · ${env.data.system.osVersion}` : ""}`} />
            <EnvFact label="架构" value={env.data.system.architecture} />
            <EnvFact label="AgentFlow" value={env.data.system.agentflowVersion} />
            <EnvFact label="Shell" value={env.data.system.shell ?? "未检测到"} ok={!!env.data.system.shell} />
            <EnvFact label="Node" value={env.data.node.found ? env.data.node.version ?? "已安装" : env.data.node.problem ?? "未安装"} ok={env.data.node.found} />
            <EnvFact label="Bun" value={env.data.bun.found ? env.data.bun.version ?? "已安装" : env.data.bun.problem ?? "未安装"} ok={env.data.bun.found} />
            <EnvFact label="网络" value={env.data.system.network.detail ?? env.data.system.network.problem ?? "未检测"} ok={env.data.system.network.available} />
            <EnvFact label="磁盘可用" value={formatBytes(env.data.system.diskAvailableBytes ?? 0)} ok={(env.data.system.diskAvailableBytes ?? 0) > 100 * 1024 * 1024} />
            <EnvFact label="系统钥匙串" value={env.data.system.keychain.detail ?? env.data.system.keychain.problem ?? "未检测"} ok={env.data.system.keychain.available} />
          </div>

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
        </div>
      )}
    </section>
  );
}

function ReadinessCard({ title, ready, detail }: { title: string; ready: boolean; detail: string }) {
  return (
    <div className={cn("rounded-[var(--radius-control)] border px-3 py-2", ready ? "border-ok/30 bg-ok/5" : "border-human/40 bg-human-bg")}>
      <div className="flex items-center gap-2 text-[13px] font-medium">
        <span className={cn("size-2 rounded-full", ready ? "bg-ok" : "bg-human")} /> {title}
      </div>
      <p className="mt-1 text-[12px] text-t3">{detail}</p>
    </div>
  );
}

function EnvFact({ label, value, ok = true }: { label: string; value: string; ok?: boolean }) {
  return (
    <div className="grid grid-cols-[6.5rem_1fr] gap-2">
      <span className="text-t3">{label}</span>
      <span className={cn("min-w-0 break-words", ok ? "text-t1" : "text-human")}>{value}</span>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "无法读取";
  const gib = bytes / 1024 / 1024 / 1024;
  return `${gib >= 1 ? gib.toFixed(1) : (bytes / 1024 / 1024).toFixed(0)} ${gib >= 1 ? "GB" : "MB"}`;
}
