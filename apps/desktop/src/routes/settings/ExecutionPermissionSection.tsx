import { ShieldCheck, ShieldX } from "lucide-react";
import { useProjectSettings, useUpdateProjectSettings } from "@/hooks/useSettings";
import { Button } from "@/components/ui/button";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

/**
 * 执行权限（原「完全放权模式」）(06 §9 第 1/2 条)。默认展示「自动执行，受限沙箱」，不再提供
 * 永久「关闭所有沙箱」开关。若检测到旧的永久 fullAccess 设置（升级后应已由后端失效），这里
 * 要求用户重新确认改用受限沙箱，绝不静默保留。
 */
export function ExecutionPermissionSection({ projectId }: { projectId: string }) {
  const settings = useProjectSettings(projectId);
  const update = useUpdateProjectSettings(projectId);
  const legacyFullAccess = !!settings.data?.fullAccess;

  const acknowledge = async () => {
    if (!settings.data) return;
    try {
      await update.mutateAsync({ ...settings.data, fullAccess: false });
      toast.info("已改用受限沙箱执行");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <div className="rounded-[var(--radius-panel)] border border-line bg-app p-3">
      <div className="mb-2 flex items-center gap-2 font-semibold">
        <ShieldCheck className="size-4 text-ok" /> 执行权限
      </div>
      <div className="flex items-start gap-2 rounded-md border border-ok/40 bg-ok/5 px-3 py-2 text-[13px]">
        <ShieldCheck className="mt-0.5 size-4 shrink-0 text-ok" aria-hidden />
        <div>
          <p className="font-medium text-t1">自动执行，受限沙箱</p>
          <p className="mt-1 text-[12px] text-t2">
            安全动作（读写任务文件、已批准的命令）自动执行；越界动作会被 AgentFlow 统一暂停、解释并请你逐项授权。
            CLI 不再自己弹出终端确认，主机其它文件、密钥与网络默认不可达。
          </p>
        </div>
      </div>

      {legacyFullAccess && (
        <div className="mt-3 flex items-start justify-between gap-3 rounded-md border border-human/60 bg-human-bg px-3 py-2 text-[13px] text-human">
          <div className="flex items-start gap-2">
            <ShieldX className="mt-0.5 size-4 shrink-0" aria-hidden />
            <div>
              <p className="font-medium">检测到旧的永久完全放权设置</p>
              <p className="mt-1 text-[12px]">
                升级后永久「关闭所有沙箱」已不再受支持，必须重新确认。确认后该项目将改用受限沙箱执行。
              </p>
            </div>
          </div>
          <Button variant="human" size="sm" onClick={acknowledge} disabled={update.isPending}>
            确认改用受限沙箱
          </Button>
        </div>
      )}

      <p className="mt-2 text-[11px] text-t3">
        完全放权仅保留为单任务、限时的高级应急能力，不再作为永久项目开关。
      </p>
    </div>
  );
}
