import { useEffect, useMemo, useRef, useState } from "react";
import { RotateCcw } from "lucide-react";
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
  canGrant,
  scopeOptions,
} from "@/lib/permission/model";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { PermissionRequestCard } from "@/components/permission/PermissionRequestCard";
import { PermissionRuleForm } from "@/components/permission/PermissionRuleForm";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { roleLabel } from "@/copy/permission";
import { PixelBot, botTint, memberShortLabel } from "@/components/pixel/PixelBot";
import { PixelIcon, type PixelIconName } from "@/components/pixel/PixelIcon";

const SCOPE_ICON: Partial<Record<PermissionGrantScope, PixelIconName>> = {
  task: "list",
  project_rule: "shield",
};

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
      const label = decision === "approve" ? "已放行，它继续干活了" : decision === "deny" ? "已告诉它换个办法" : "任务已取消";
      toast.info(label);
      setSub({ kind: "decide" });
      setGuidance("");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  const mergedCount = active.requests.length;
  const who = memberShortLabel(request.providerId);
  const restrictedNote = isProjectRuleRestricted(request) && request.riskLevel !== "high";

  const footer =
    sub.kind === "ruleConfirm" ? (
      <>
        <Button variant="outline" onClick={() => setSub({ kind: "decide" })} disabled={busy}>
          <PixelIcon name="chevronLeft" size={9} /> 返回
        </Button>
        <Button variant="human" onClick={() => submit("approve", "project_rule")} disabled={approveBlocked}>
          <PixelIcon name="shield" size={10} /> 确认保存并允许
        </Button>
      </>
    ) : sub.kind === "deny" ? (
      <>
        <Button variant="outline" onClick={() => setSub({ kind: "decide" })} disabled={busy}>
          <PixelIcon name="chevronLeft" size={9} /> 返回
        </Button>
        <Button variant="danger" onClick={() => submit("deny", "once")} disabled={busy}>
          <PixelIcon name="swap" size={10} /> 拒绝并告诉它
        </Button>
      </>
    ) : (
      <div className="flex w-full flex-wrap items-center gap-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => submit("cancel_task", "once")}
          disabled={busy}
          title="整个任务停下来，不再继续"
          className="text-t3"
        >
          <PixelIcon name="stop" size={9} /> 取消任务
        </Button>
        <div className="ml-auto flex flex-wrap items-center justify-end gap-2">
          <Button
            variant="danger"
            size="sm"
            onClick={() => setSub({ kind: "deny" })}
            disabled={busy}
            title="不放行，告诉它换个办法；任务不会因此失败"
          >
            <PixelIcon name="swap" size={10} /> 换个办法
          </Button>
          {scopes.map((scope) => {
            const isRule = scope === "project_rule";
            return (
              <Button
                key={scope}
                variant={scope === "once" ? "primary" : "human"}
                size="sm"
                disabled={approveBlocked}
                title={SCOPE_COPY[scope].hint}
                onClick={() => (isRule ? setSub({ kind: "ruleConfirm" }) : submit("approve", scope))}
              >
                {scope === "once" ? (
                  <span className="font-pixel text-micro" aria-hidden>×1</span>
                ) : SCOPE_ICON[scope] ? (
                  <PixelIcon name={SCOPE_ICON[scope]!} size={10} />
                ) : null}
                {SCOPE_BUTTON[scope]}
              </Button>
            );
          })}
        </div>
      </div>
    );

  const subtitle = (
    <span className="flex items-center gap-1.5">
      <span>{roleLabel(request.role)}阶段</span>
      {!expiry.expired && expiry.remainingLabel && (
        <span className="inline-flex items-center gap-1 rounded-row bg-raised/70 px-1.5 py-px font-semibold text-t1" title="到期前它会停在这一步等你">
          <PixelIcon name="hourglass" size={9} /> {expiry.remainingLabel}内
        </span>
      )}
      {groups.length > 1 && (
        <span className="inline-flex items-center gap-1 rounded-row bg-raised/70 px-1.5 py-px font-semibold text-t1" title="不同的请求会一件一件问你">
          <PixelIcon name="list" size={9} /> 还有 {groups.length - 1} 件
        </span>
      )}
    </span>
  );

  return (
    <Dialog
      open
      onClose={onClose}
      title={sub.kind === "deny" ? `告诉 ${who} 换个办法` : sub.kind === "ruleConfirm" ? "存成项目规则前，再确认一下" : canGrant(request) ? `${who} 想请你点个头` : `${who} 想做一件不能放行的事`}
      subtitle={subtitle}
      leading={<PixelBot member={request.providerId} mode="hand" size={44} />}
      headerTint={botTint(request.providerId)}
      width={640}
      footer={footer}
    >
      {stale ? (
        <div className="flex flex-col items-start gap-2 rounded-section bg-status-human-bg px-3.5 py-3 text-body text-status-human">
          <div className="flex items-center gap-1.5 font-bold">
            <PixelIcon name="alert" size={12} /> 它要做的事刚刚变了
          </div>
          <p className="text-meta text-t2">为了安全，刚才的按钮已经失效。看一眼新内容再决定。</p>
          <Button variant="human" size="sm" onClick={reload}>
            <RotateCcw className="size-3.5" aria-hidden /> 看新内容
          </Button>
        </div>
      ) : (
        <>
          {expiry.expired && (
            <div className="mb-3 flex items-center gap-2 rounded-control bg-status-idle-bg px-3 py-2 text-meta text-t2">
              <PixelIcon name="hourglass" size={11} className="shrink-0 text-t3" />
              等太久，这个请求已过期。它需要的话会重新举手。
            </div>
          )}

          {sub.kind === "ruleConfirm" ? (
            <PermissionRuleForm request={request} />
          ) : sub.kind === "deny" ? (
            <DenyGuidance value={guidance} onChange={setGuidance} />
          ) : (
            <>
              <PermissionRequestCard request={request} />
              {(mergedCount > 1 || restrictedNote) && (
                <div className="mt-3 flex flex-wrap gap-x-3 gap-y-1 text-meta text-t3">
                  {mergedCount > 1 && (
                    <span className="inline-flex items-center gap-1.5">
                      <PixelIcon name="squad" size={10} /> 同样的请求有 {mergedCount} 个，一起处理（每次仍单独记录）
                    </span>
                  )}
                  {restrictedNote && (
                    <span className="inline-flex items-center gap-1.5">
                      <PixelIcon name="lock" size={10} /> 这类请求不能存成项目规则
                    </span>
                  )}
                </div>
              )}
            </>
          )}
        </>
      )}
    </Dialog>
  );
}

const SCOPE_BUTTON: Record<PermissionGrantScope, string> = {
  once: "允许这次",
  task: "本任务都允许",
  project_rule: "存为项目规则",
};

/** 一键填入的常用指引，点了还能再改。 */
const GUIDANCE_PRESETS = [
  "别装新依赖，用项目里已有的",
  "不要碰工作树外的文件",
  "这一步我来手动做，你先跳过",
  "先停一下，说说还有什么办法",
];

function DenyGuidance({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <div className="flex flex-col gap-2.5">
      <p className="flex items-center gap-2 text-body text-t2">
        <PixelIcon name="check" size={11} className="shrink-0 text-status-success" />
        放心，拒绝不会让任务失败。说一句怎么做，它会换个安全的办法。
      </p>
      <div className="flex flex-wrap gap-1.5" aria-label="常用指引">
        {GUIDANCE_PRESETS.map((preset) => (
          <button
            key={preset}
            type="button"
            onClick={() => onChange(value.trim() ? `${value.trim()}；${preset}` : preset)}
            className="rounded-pill border border-line-strong bg-raised px-2.5 py-1 text-meta font-semibold text-t2 transition-colors hover:bg-app hover:text-t1"
          >
            {preset}
          </button>
        ))}
      </div>
      <Textarea
        autoFocus
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="也可以自己写，例如：改用项目内的类型定义。"
        className="min-h-20"
      />
    </div>
  );
}
