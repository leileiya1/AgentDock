import type { ReactNode } from "react";
import { ShieldAlert, ShieldBan, ShieldCheck, ShieldQuestion } from "lucide-react";
import type { PermissionRequestStatus, PermissionRiskLevel } from "@/generated/bindings";
import { requestStatusCopy, riskCopy, type RiskTone } from "@/copy/permission";
import { cn } from "@/lib/utils";

/**
 * 风险 / 状态徽标 (06 §9 第 22 条). 绝不只用颜色：每个徽标同时带图标与文字，
 * 黑白截图、色弱模式、读屏都能区分。tone 只是补充语义色。
 */

const TONE_CLS: Record<RiskTone | "idle", string> = {
  ok: "text-status-success border-status-success/40",
  caution: "text-caution border-caution/45 bg-caution-bg/70",
  human: "text-status-human border-status-human/60 bg-status-human-bg",
  bad: "text-status-danger border-status-danger/40",
  idle: "text-status-idle border-line",
};

function Pill({ tone, icon, children, className }: { tone: RiskTone | "idle"; icon?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 whitespace-nowrap rounded-pill border px-2 py-0.5 text-meta font-medium leading-none",
        TONE_CLS[tone],
        className
      )}
    >
      {icon}
      {children}
    </span>
  );
}

const RISK_ICON: Record<PermissionRiskLevel, ReactNode> = {
  low: <ShieldCheck className="size-3" aria-hidden />,
  medium: <ShieldAlert className="size-3" aria-hidden />,
  high: <ShieldAlert className="size-3" aria-hidden />,
  forbidden: <ShieldBan className="size-3" aria-hidden />,
};

export function RiskBadge({ risk, className }: { risk: PermissionRiskLevel; className?: string }) {
  const copy = riskCopy(risk);
  return (
    <Pill tone={copy.tone} icon={RISK_ICON[risk] ?? <ShieldQuestion className="size-3" aria-hidden />} className={className}>
      <span>风险：{copy.label}</span>
    </Pill>
  );
}

export function RequestStatusBadge({ status, className }: { status: PermissionRequestStatus; className?: string }) {
  const copy = requestStatusCopy(status);
  const icon =
    status === "approved" ? <ShieldCheck className="size-3" aria-hidden /> :
    status === "denied" ? <ShieldBan className="size-3" aria-hidden /> :
    status === "pending" ? <ShieldAlert className="size-3" aria-hidden /> :
    <ShieldQuestion className="size-3" aria-hidden />;
  return <Pill tone={copy.tone} icon={icon} className={className}>{copy.label}</Pill>;
}

export { Pill as PermissionPill };
