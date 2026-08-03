import { Ban, MapPin, MessageSquareQuote } from "lucide-react";
import type { PermissionRequest } from "@/generated/bindings";
import { actionCopy, riskCopy, roleLabel } from "@/copy/permission";
import { agentLabel } from "@/copy/agents";
import { canGrant } from "@/lib/permission/model";
import { AgentMark } from "@/components/AgentMark";
import { RiskBadge } from "@/components/permission/PermissionBadge";
import { PermissionOperationView } from "@/components/permission/PermissionOperationView";

/**
 * 授权请求正文 (06 §9 第 5/12 条)。只展示用户能判断的内容：谁、想做什么、在哪执行、
 * 会访问什么、为什么需要。不可授权请求隐藏一切允许入口，转而解释原因与安全替代方案。
 */
export function PermissionRequestCard({ request }: { request: PermissionRequest }) {
  const action = actionCopy(request.actionType);
  const risk = riskCopy(request.riskLevel);
  const grantable = canGrant(request);

  return (
    <div className="flex flex-col gap-3">
      {/* 谁 + 想做什么 + 风险 */}
      <div className="flex items-start gap-2.5">
        <AgentMark kind={request.providerId} size={30} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="text-body font-semibold text-t1">{agentLabel(request.providerId)}</span>
            <span className="rounded bg-raised px-1.5 py-0.5 text-micro text-t3">{roleLabel(request.role)} Agent</span>
            <RiskBadge risk={request.riskLevel} />
          </div>
          {/* summary 是后端生成的短标题，绝不直接用 Provider 自报文案作主标题 (§5.1)。 */}
          <p className="mt-1 text-section font-medium text-t1">{request.summary}</p>
          <p className="text-meta text-t3">{action.hint}</p>
        </div>
      </div>

      {/* 在哪执行 */}
      <div className="flex items-center gap-1.5 text-meta text-t2">
        <MapPin className="size-3.5 shrink-0 text-t3" aria-hidden />
        <span className="min-w-0 break-all font-mono">{request.operation.cwd || "（未指定目录）"}</span>
      </div>

      {/* 会访问什么 */}
      <PermissionOperationView request={request} />

      {/* 为什么需要 */}
      <div className="flex items-start gap-1.5 rounded-control border border-line bg-app px-2.5 py-2 text-meta text-t2">
        <MessageSquareQuote className="mt-0.5 size-3.5 shrink-0 text-t3" aria-hidden />
        <span className="min-w-0">{request.reason}</span>
      </div>

      {/* 不可授权：解释 + 安全替代，绝不给允许按钮 (§9 第 12 条) */}
      {!grantable && (
        <div className="flex items-start gap-2 rounded-control border border-bad/40 bg-bad/5 px-2.5 py-2 text-meta text-bad">
          <Ban className="mt-0.5 size-4 shrink-0" aria-hidden />
          <div className="min-w-0">
            <p className="font-medium">这项操作不可授权：{risk.why}</p>
            <p className="mt-1 text-t2">
              请拒绝并补充指引，让 Agent 换用工作树内的安全方案（例如改用项目脚本、请你手动完成系统级步骤，或不依赖该能力）。
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
