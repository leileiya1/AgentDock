import { AlertTriangle, CheckCircle2, XCircle } from "lucide-react";
import type { TaskPreflightReport } from "@/generated/bindings";
import { summarizePreflight, type PreflightRoleView, type PreflightView } from "@/lib/preflight";
import { Dialog } from "./Dialog";
import { Button } from "@/components/ui/button";

interface Props {
  open: boolean;
  report: TaskPreflightReport | null;
  redetecting: boolean;
  onRedetect: () => void;
  onClose: () => void;
}

/**
 * Preflight 阻断：创建并立即开始之前，若开发或审查角色的整条降级链都没有可运行的
 * Provider，就不创建注定失败的运行，而是在这里一次性列出每个 Provider 不可用的原因，
 * 并给出「重新检测」入口（P0-01/P0-02）。
 */
export function PreflightBlockedDialog({ open, report, redetecting, onRedetect, onClose }: Props) {
  if (!report) return null;
  const view = summarizePreflight(report);
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="还不能开始 · 环境未就绪"
      width={520}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>知道了</Button>
          <Button variant="primary" disabled={redetecting} onClick={onRedetect}>
            {redetecting ? "检测中…" : "重新检测"}
          </Button>
        </>
      }
    >
      <PreflightReportBody view={view} />
    </Dialog>
  );
}

/** 纯展示子组件：便于用 renderToStaticMarkup 做无 DOM 回归测试。 */
export function PreflightReportBody({ view }: { view: PreflightView }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2 rounded-md border border-human/50 bg-human-bg px-3 py-2 text-[13px] leading-5 text-t1">
        <AlertTriangle className="mt-0.5 size-4 shrink-0 text-human" />
        <span>
          「已连接」只代表命令存在；这里做了一次真实的登录与协议探测。修复下面标红的 Provider 后点
          「重新检测」，就绪即可从这里开始，<span className="font-medium">无需重新创建任务</span>。
        </span>
      </div>
      {view.roles.map((role) => (
        <RoleSection key={role.role} role={role} />
      ))}
      <p className="text-[12px] leading-5 text-t3">
        提示：CLI 未登录时在终端里重新登录（如 <code className="font-mono">claude</code>）；
        未安装时到「设置 · Provider」安装或填写可执行文件路径；钥匙串问题按提示修复后再重新检测。
      </p>
    </div>
  );
}

function RoleSection({ role }: { role: PreflightRoleView }) {
  return (
    <div className="rounded-md border border-line bg-app/60 p-3">
      <div className="mb-2 flex items-center justify-between gap-3">
        <div className="text-[13px] font-medium text-t1">
          {role.label}角色 · 首选 {role.primaryLabel}
        </div>
        {role.ready ? (
          <span className="flex items-center gap-1 text-[12px] text-ok">
            <CheckCircle2 className="size-3.5" /> 至少一个可运行
          </span>
        ) : (
          <span className="flex items-center gap-1 text-[12px] text-human">
            <XCircle className="size-3.5" /> 无可运行 Provider
          </span>
        )}
      </div>
      <ul className="flex flex-col gap-1.5">
        {role.providers.map((provider) => (
          <li key={provider.key} className="flex items-start gap-2 text-[12px] leading-5">
            <span
              className={`mt-1.5 size-2 shrink-0 rounded-full ${provider.available ? "bg-ok" : "bg-human"}`}
              aria-hidden
            />
            <span className="text-t2">
              <span className={provider.available ? "text-t1" : "text-t1"}>{provider.label}</span>
              {provider.available ? (
                <span className="ml-1 text-ok">可运行</span>
              ) : (
                <span className="ml-1 text-t3">{provider.problem ?? "当前不可用"}</span>
              )}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
