import { useState } from "react";
import { ShieldAlert, ShieldCheck } from "lucide-react";
import {
  useApproveProjectConfig,
  useProjectConfigTrust,
  useRevokeProjectConfig,
} from "@/hooks/useSettings";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

export function ProjectConfigTrustPanel({ projectId }: { projectId: string }) {
  const trust = useProjectConfigTrust(projectId);
  const approve = useApproveProjectConfig(projectId);
  const revoke = useRevokeProjectConfig(projectId);
  const [confirmOpen, setConfirmOpen] = useState(false);

  if (trust.isError) return <ErrorState error={trust.error} onRetry={() => trust.refetch()} compact />;
  if (trust.isLoading || !trust.data) return <SkeletonRows rows={2} />;

  const value = trust.data;
  if (!value.exists) {
    return (
      <div className="rounded-[var(--radius-panel)] border border-line bg-app p-3 text-[12px] text-t3">
        当前项目没有 <span className="font-mono">.agentflow/project.toml</span>，没有仓库命令需要授权。
      </div>
    );
  }

  const approveCurrent = async () => {
    try {
      await approve.mutateAsync();
      setConfirmOpen(false);
      toast.info("已信任当前版本的项目配置");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const revokeCurrent = async () => {
    try {
      await revoke.mutateAsync();
      toast.info("已撤销项目配置授权");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <>
      <div className={`rounded-[var(--radius-panel)] border p-3 ${value.trusted ? "border-ok/40 bg-app" : "border-human/50 bg-human-bg"}`}>
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2 font-semibold">
              {value.trusted ? <ShieldCheck className="size-4 text-ok" /> : <ShieldAlert className="size-4 text-human" />}
              仓库配置权限
            </div>
            <p className="mt-1 text-[12px] text-t3">
              {value.trusted ? "当前文件与本机批准的 SHA-256 一致。" : "配置未批准或批准后已变更，任务启动与命令执行已被阻止。"}
            </p>
          </div>
          {value.trusted ? (
            <Button variant="outline" size="sm" onClick={revokeCurrent} disabled={revoke.isPending}>撤销</Button>
          ) : (
            <Button variant="human" size="sm" onClick={() => setConfirmOpen(true)}>检查并批准…</Button>
          )}
        </div>
        <div className="mt-2 break-all font-mono text-[11px] text-t3">SHA-256 {value.sha256}</div>
        {!!value.validationSteps.length && <div className="mt-2 text-[12px] text-t2">验证步骤：{value.validationSteps.join("、")}</div>}
        {!!value.extraAllowedCommands.length && <div className="mt-1 text-[12px] text-human">额外命令权限：{value.extraAllowedCommands.join("、")}</div>}
      </div>

      <Dialog
        open={confirmOpen}
        onClose={() => setConfirmOpen(false)}
        title="批准当前项目配置"
        onConfirmKey={approveCurrent}
        footer={
          <>
            <Button variant="outline" onClick={() => setConfirmOpen(false)}>取消</Button>
            <Button variant="human" onClick={approveCurrent} disabled={approve.isPending}>
              {approve.isPending ? "批准中…" : "只批准这个 SHA-256"}
            </Button>
          </>
        }
      >
        <p className="text-[13px] text-t2">批准后，AgentFlow 可以执行下列仓库提供的验证步骤和额外 CLI 权限。文件只要变化一个字节，授权就会自动失效。</p>
        <div className="mt-3 rounded-md border border-line bg-raised p-3 text-[12px]">
          <div>验证：{value.validationSteps.length ? value.validationSteps.join("、") : "无"}</div>
          <div className="mt-1">额外权限：{value.extraAllowedCommands.length ? value.extraAllowedCommands.join("、") : "无"}</div>
        </div>
      </Dialog>
    </>
  );
}
