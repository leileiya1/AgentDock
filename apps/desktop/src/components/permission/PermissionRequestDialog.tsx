import { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, Clock, Info, Layers, RotateCcw } from "lucide-react";
import type {
  PermissionDecisionKind,
  PermissionGrantScope,
  PermissionRequest,
} from "@/generated/bindings";
import { SCOPE_COPY } from "@/copy/permission";
import { usePermissionDecide } from "@/hooks/usePermissions";
import {
  expiryInfo,
  groupPendingRequests,
  isProjectRuleRestricted,
  isStale,
  scopeOptions,
} from "@/lib/permission/model";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { PermissionRequestCard } from "@/components/permission/PermissionRequestCard";
import { PermissionRuleForm } from "@/components/permission/PermissionRuleForm";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";

interface Props {
  taskId: string;
  projectId: string | undefined;
  requests: PermissionRequest[];
  open: boolean;
  onClose: () => void;
}

type SubMode = { kind: "decide" } | { kind: "ruleConfirm" } | { kind: "deny" };

/**
 * 统一授权弹窗 (06 §9 第 10–16 条)。经 body portal 居中 (Dialog)，一次处理一组「操作摘要 +
 * 权限完全相同」的请求；打开后内容变化立即作废并重新加载；到期前停在动作之前；拒绝走补充
 * 指引；「保存项目规则」二次展示实际规则。所有决定都调用真实 permission_decide，绝不前端模拟。
 */
export function PermissionRequestDialog({ taskId, projectId, requests, open, onClose }: Props) {
  const groups = useMemo(() => groupPendingRequests(requests), [requests]);
  const active = groups[0];
  const decide = usePermissionDecide(taskId, projectId);

  const [sub, setSub] = useState<SubMode>({ kind: "decide" });
  const [guidance, setGuidance] = useState("");
  // 打开这一组时的快照，用来检测 TOCTOU 内容变化 (§9 第 15 条)。
  const [shownAt, setShownAt] = useState<PermissionRequest | null>(null);
  const shownIdRef = useRef<string | null>(null);

  // 切换到新的一组请求时重置子模式并重新快照。
  useEffect(() => {
    if (!active) {
      shownIdRef.current = null;
      setShownAt(null);
      return;
    }
    if (shownIdRef.current !== active.representative.id) {
      shownIdRef.current = active.representative.id;
      setShownAt(active.representative);
      setSub({ kind: "decide" });
      setGuidance("");
    }
  }, [active]);

  // 每秒推进倒计时，仅在弹窗打开时运行（纯文本，不涉及动画）。
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [active]);

  if (!active || !shownAt || !open) return null;

  const request = active.representative;
  const live = requests.find((r) => r.id === shownAt.id);
  const stale = isStale(shownAt, live);
  const expiry = expiryInfo(request, now);
  const scopes = scopeOptions(request);
  const busy = decide.isPending;
  // 内容变化或已过期时，一切允许按钮立即不可用 (§9 第 15/16 条)。
  const approveBlocked = stale || expiry.expired || busy;

  const reload = () => {
    setShownAt(live ?? request);
    setSub({ kind: "decide" });
  };

  const submit = async (decision: PermissionDecisionKind, scope: PermissionGrantScope) => {
    try {
      // 合并组内每条请求都提交决定，保留各自审计 (§9 第 14 条)。cancel 只需一次。
      const targets = decision === "cancel_task" ? [request] : active.requests;
      for (const target of targets) {
        await decide.mutateAsync({
          requestId: target.id,
          operationSha256: target.operationSha256,
          policySha256: target.policySha256,
          decision,
          scope,
          guidance: decision === "deny" ? guidance.trim() || null : null,
        });
      }
      const label = decision === "approve" ? "已授权" : decision === "deny" ? "已拒绝并提交指引" : "已取消任务";
      toast.info(label);
      setSub({ kind: "decide" });
      setGuidance("");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  const mergedNote = active.requests.length > 1;

  const footer =
    sub.kind === "ruleConfirm" ? (
      <>
        <Button variant="outline" onClick={() => setSub({ kind: "decide" })} disabled={busy}>返回</Button>
        <Button variant="human" onClick={() => submit("approve", "project_rule")} disabled={approveBlocked}>
          确认保存并允许
        </Button>
      </>
    ) : sub.kind === "deny" ? (
      <>
        <Button variant="outline" onClick={() => setSub({ kind: "decide" })} disabled={busy}>返回</Button>
        <Button variant="danger" onClick={() => submit("deny", "once")} disabled={busy}>确认拒绝</Button>
      </>
    ) : (
      <div className="flex w-full flex-wrap items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={() => submit("cancel_task", "once")} disabled={busy}>
          取消任务
        </Button>
        <Button variant="danger" size="sm" onClick={() => setSub({ kind: "deny" })} disabled={busy}>
          拒绝并补充指引
        </Button>
        {scopes.map((scope) => {
          const isRule = scope === "project_rule";
          return (
            <Button
              key={scope}
              variant={scope === "once" ? "primary" : "human"}
              size="sm"
              disabled={approveBlocked}
              onClick={() => (isRule ? setSub({ kind: "ruleConfirm" }) : submit("approve", scope))}
            >
              {SCOPE_COPY[scope].label}
            </Button>
          );
        })}
      </div>
    );

  return (
    <Dialog
      open
      onClose={onClose}
      title="等待你授权一项执行权限"
      width={560}
      footer={footer}
    >
      {groups.length > 1 && (
        <div className="mb-3 flex items-center gap-1.5 rounded-md border border-line bg-app px-2.5 py-1.5 text-[12px] text-t2">
          <Layers className="size-3.5 shrink-0 text-t3" aria-hidden />
          还有 {groups.length - 1} 组不同的请求待处理，将逐个确认。
        </div>
      )}

      {stale ? (
        <div className="flex flex-col items-start gap-2 rounded-md border border-human/60 bg-human-bg px-3 py-2.5 text-[13px] text-human">
          <div className="flex items-center gap-1.5 font-medium">
            <AlertTriangle className="size-4 shrink-0" aria-hidden /> 请求内容已变化
          </div>
          <p className="text-[12px]">打开弹窗后操作、策略或任务已改变，原来的批准按钮已作废。请重新加载后再决定。</p>
          <Button variant="human" size="sm" onClick={reload}>
            <RotateCcw className="size-3.5" aria-hidden /> 重新加载
          </Button>
        </div>
      ) : (
        <>
          {expiry.expired && (
            <div className="mb-3 flex items-center gap-1.5 rounded-md border border-line bg-app px-2.5 py-1.5 text-[12px] text-t2">
              <Clock className="size-3.5 shrink-0" aria-hidden /> 该请求已过期，需 Agent 重新发起后才能授权。
            </div>
          )}
          {!expiry.expired && expiry.remainingLabel && (
            <div className="mb-3 flex items-center gap-1.5 text-[12px] text-t3">
              <Clock className="size-3.5 shrink-0" aria-hidden /> 剩余有效期 {expiry.remainingLabel}，到期前会停在动作之前。
            </div>
          )}
          {mergedNote && (
            <div className="mb-3 flex items-center gap-1.5 rounded-md border border-line bg-app px-2.5 py-1.5 text-[12px] text-t2">
              <Info className="size-3.5 shrink-0 text-t3" aria-hidden />
              这组包含 {active.requests.length} 个完全相同的请求，将一并处理（每次执行仍单独审计）。
            </div>
          )}

          {sub.kind === "ruleConfirm" ? (
            <PermissionRuleForm request={request} />
          ) : sub.kind === "deny" ? (
            <div className="flex flex-col gap-2">
              <p className="text-[13px] text-t2">拒绝不会让任务直接失败。补充一句指引，Agent 会尝试换用安全方案。</p>
              <Textarea
                autoFocus
                value={guidance}
                onChange={(e) => setGuidance(e.target.value)}
                placeholder="例如：不要访问工作树外文件，改用项目内的类型定义。"
                className="min-h-20"
              />
            </div>
          ) : (
            <>
              <PermissionRequestCard request={request} />
              {isProjectRuleRestricted(request) && (
                <p className="mt-3 flex items-center gap-1.5 text-[12px] text-t3">
                  <Info className="size-3.5 shrink-0" aria-hidden />
                  高风险请求只提供「允许一次 / 本任务允许」，不能保存为项目规则。
                </p>
              )}
            </>
          )}
        </>
      )}
    </Dialog>
  );
}
