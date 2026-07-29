import type { TaskStatus } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { PHASE_LABEL, type NodeState } from "@/copy/events";
import type { ExecutionTree } from "./tree";
import { isTaskExecuting, isTaskTerminal } from "@/lib/taskStatus";

/**
 * 当前运行状态必须持续可见 (05 §3.4). Pure derivation so the状态条 never disagrees with
 * the tree, and so "仍在运行" vs "卡死" is decided by data rather than by animation.
 */
export interface LiveStatus {
  tone: NodeState;
  /** 例：`Codex 正在开发`、`审查委员会进行中`。 */
  headline: string;
  /** 例：`2/3 成员已返回`、`已降级到 DeepSeek API`。 */
  detail: string | null;
  /** 本步骤已用时（秒）；未知为 null。 */
  elapsedSecs: number | null;
  /** 最近一次有内容的活动时间。 */
  lastActivityAt: string | null;
  /**
   * 进程仍存活但超过 10 秒没有新活动。UI 必须显示「仍在运行，最近活动于 …」，
   * 不能表现为静止或卡死 (05 §3.4)。
   */
  stalled: boolean;
}

const IDLE_THRESHOLD_MS = 10_000;
const WAITING_COPY: Partial<Record<TaskStatus, { headline: string; detail: string | null }>> = {
  WAITING_FOR_PLAN_APPROVAL: { headline: "等待你批准编码计划", detail: "任务已暂停，批准后才会开始写代码" },
  WAITING_FOR_HUMAN_APPROVAL: { headline: "等待你确认本轮改动", detail: "确认后才会进入交付" },
  READY_FOR_DEVELOPMENT: { headline: "排队等待开发", detail: "等待调度分配执行位" },
  READY_FOR_REVIEW: { headline: "排队等待审查", detail: "等待调度分配执行位" },
  READY_FOR_REVISION: { headline: "排队等待返工", detail: "等待调度分配执行位" },
  APPROVED: { headline: "已批准，等待合并", detail: null },
  MERGE_CONFLICT: { headline: "合并冲突，需要你处理", detail: "目标分支已前进，自动合并无法继续" },
  BLOCKED: { headline: "需要你处理", detail: "任务已安全停在检查点" },
};

export interface LiveStatusInput {
  tree: ExecutionTree;
  status: TaskStatus;
  /** 最近一条日志/事件时间，用于判定「仍在运行」。 */
  lastActivityAt: string | null;
  now?: number;
}

export function liveStatus(input: LiveStatusInput): LiveStatus | null {
  const { tree, status, lastActivityAt } = input;
  // A persisted task terminal state is authoritative even if an older event snapshot
  // still contains a running transition. Never keep the spinner alive after delivery.
  if (isTaskTerminal(status)) return null;

  const now = input.now ?? Date.now();
  const current = tree.revisions.find((r) => r.revision === tree.currentRevision);

  // Persisted task state wins over a delayed run/event snapshot. A blocked,
  // queued or approval-waiting task must never look as if its old process is live.
  const runningPhase = isTaskExecuting(status)
    ? current?.phases.find((p) => p.state === "running")
    : undefined;
  if (runningPhase) {
    const runningGroups = runningPhase.groups.filter((g) => g.state === "running");
    const active = runningGroups[0]?.current ?? runningPhase.groups.at(-1)?.current;
    const agent = active?.agent ? agentLabel(active.agent) : null;
    const isCouncil = runningPhase.phase === "review" && runningPhase.groups.length > 1;
    const returned = runningPhase.groups.filter((g) => g.state !== "running" && g.state !== "pending").length;

    const started = active?.startedAt ?? runningPhase.startedAt;
    const elapsedSecs = started ? Math.max(0, Math.round((now - Date.parse(started)) / 1000)) : null;
    const activity = lastActivityAt ?? started;

    const detailParts: string[] = [];
    if (isCouncil) detailParts.push(`${returned}/${runningPhase.groups.length} 成员已返回`);
    if (active && active.attemptLabel !== "首选") {
      detailParts.push(`${active.attemptLabel}${active.fallbackReason ? ` · ${active.fallbackReason}` : ""}`);
    }
    const adopting = active?.recovery.at(-1);
    if (adopting?.copy.state === "running") detailParts.push("后台服务已重连，正在确认仍运行的 Agent");

    return {
      tone: "running",
      headline: isCouncil
        ? `审查委员会进行中`
        : agent
          ? `${agent} 正在${PHASE_LABEL[runningPhase.phase]}`
          : `正在${PHASE_LABEL[runningPhase.phase]}`,
      detail: detailParts.join(" · ") || null,
      elapsedSecs,
      lastActivityAt: activity,
      stalled: !!activity && now - Date.parse(activity) > IDLE_THRESHOLD_MS,
    };
  }

  const waiting = WAITING_COPY[status];
  if (waiting) {
    return {
      tone: status === "READY_FOR_DEVELOPMENT" || status === "READY_FOR_REVIEW" || status === "READY_FOR_REVISION"
        ? "pending"
        : "attention",
      headline: waiting.headline,
      detail: waiting.detail ?? null,
      elapsedSecs: null,
      lastActivityAt,
      stalled: false,
    };
  }

  return null;
}

/** `02:41` / `1:12:05`——步骤耗时始终可见，不用「刚刚」这种模糊说法。 */
export function formatElapsed(seconds: number | null): string {
  if (seconds == null) return "—";
  const s = Math.max(0, Math.floor(seconds));
  const hh = Math.floor(s / 3600);
  const mm = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return hh > 0 ? `${hh}:${pad(mm)}:${pad(ss)}` : `${pad(mm)}:${pad(ss)}`;
}
