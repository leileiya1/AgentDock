import type { PermissionRequest, PermissionRiskLevel } from "@/generated/bindings";
import { riskCopy, roleLabel } from "@/copy/permission";
import { canGrant } from "@/lib/permission/model";
import { PermissionOperationView } from "@/components/permission/PermissionOperationView";
import { PixelIcon, type PixelIconName } from "@/components/pixel/PixelIcon";
import { cn } from "@/lib/utils";

/**
 * 授权请求正文 (06 §9 第 5/12 条)。只展示用户能判断的内容：谁、想做什么、为什么、
 * 会碰到什么。「谁」已在弹窗标题栏（头像 + 名字），这里不重复；理由紧跟在请求后面，
 * 读起来像它在跟你说话。不可授权请求隐藏一切允许入口，转而解释原因与安全替代方案。
 */
export function PermissionRequestCard({ request }: { request: PermissionRequest }) {
  const risk = riskCopy(request.riskLevel);
  const grantable = canGrant(request);

  return (
    <div className="flex flex-col gap-3.5">
      {/* 对话气泡：想做什么 + 为什么。头像在标题栏左侧，气泡尖角朝向它。 */}
      <div className="rounded-section rounded-tl-row border border-line bg-app px-4 py-3">
        {/* summary 是后端生成的短标题，绝不直接用 Provider 自报文案作主标题 (§5.1)。 */}
        <p className="text-section font-bold leading-snug text-t1">{request.summary}</p>
        <p className="mt-1.5 flex items-start gap-1.5 text-body text-t2">
          <span className="mt-0.5 shrink-0 rounded-row bg-raised px-1.5 text-micro font-bold text-t3">因为</span>
          <span className="min-w-0">{request.reason}</span>
        </p>
        <span className="sr-only">{roleLabel(request.role)} Agent</span>
      </div>

      {grantable && <RiskLine risk={request.riskLevel} label={risk.label} why={risk.why} />}

      {/* 会碰到什么 */}
      <PermissionOperationView request={request} />

      {/* 不可授权：解释 + 安全替代，绝不给允许按钮 (§9 第 12 条) */}
      {!grantable && (
        <div className="flex items-start gap-2.5 rounded-section bg-status-danger-bg px-3.5 py-3 text-meta text-status-danger">
          <PixelIcon name="lock" size={14} className="mt-0.5 shrink-0" />
          <div className="min-w-0">
            <p className="font-bold">不可授权：{risk.why}</p>
            <p className="mt-1 text-t2">
              点「换个办法」告诉它怎么绕开：比如改用项目脚本、不依赖这个能力，或由你手动完成系统级步骤。它会改用工作树内的安全方案。
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

const RISK_STYLE: Record<PermissionRiskLevel, { icon: PixelIconName; cls: string }> = {
  low: { icon: "check", cls: "bg-status-success-bg text-status-success" },
  medium: { icon: "eye", cls: "bg-caution-bg text-caution" },
  high: { icon: "alert", cls: "bg-status-human-bg text-status-human" },
  forbidden: { icon: "lock", cls: "bg-status-danger-bg text-status-danger" },
};

/** 风险一行话：图标 + 级别 + 一句「你该怎么看」，绝不只用颜色 (§9 第 22 条)。 */
function RiskLine({ risk, label, why }: { risk: PermissionRiskLevel; label: string; why: string }) {
  const style = RISK_STYLE[risk] ?? RISK_STYLE.high;
  return (
    <div className={cn("flex items-center gap-2 rounded-control px-3 py-2 text-meta", style.cls)}>
      <PixelIcon name={style.icon} size={12} className="shrink-0" />
      <span className="shrink-0 font-bold">{label}</span>
      <span className="min-w-0 text-t2">{why}</span>
    </div>
  );
}
