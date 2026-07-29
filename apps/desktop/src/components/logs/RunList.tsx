import { useMemo } from "react";
import type { RunRole } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { PHASE_LABEL } from "@/copy/events";
import type { ExecutionTree, PhaseNode } from "@/lib/execution/tree";
import { formatElapsed } from "@/lib/execution/liveStatus";
import { AgentMark } from "@/components/AgentMark";
import { PhaseIcon, StateMark } from "@/components/execution/StateMark";
import { cn } from "@/lib/utils";

const ROLE_LABEL: Record<RunRole, string> = {
  planner: "计划",
  developer: "开发",
  reviewer: "审查",
  validator: "验证",
};

const STATUS_LABEL: Record<string, string> = {
  RUNNING: "进行中",
  SUCCEEDED: "成功",
  FAILED: "失败",
  TIMED_OUT: "超时",
  CANCELLED: "已取消",
  INTERRUPTED: "已中断",
};

export interface RunRow {
  runId: string;
  role: RunRole;
  phase: PhaseNode["phase"];
}

/**
 * 中间 run / 成员列表 (05 §5.3). 按当前阶段分组，默认只显示当前 revision——
 * 历史轮次从执行树进入，不再把所有 revision 的所有 run 一次平铺。
 */
export function RunList({
  tree,
  revision,
  selectedRunId,
  onSelect,
}: {
  tree: ExecutionTree;
  revision: number;
  selectedRunId: string | null;
  onSelect: (row: RunRow) => void;
}) {
  const phases = useMemo(
    () => tree.revisions.find((r) => r.revision === revision)?.phases.filter((p) => p.groups.length > 0) ?? [],
    [tree, revision]
  );

  return (
    <div className="flex flex-col gap-3 p-2">
      <div className="px-1 text-[11px] text-t3">r{revision} 的执行者</div>
      {phases.map((phase) => (
        <section key={phase.phase}>
          <h3 className="mb-1 flex items-center gap-1.5 px-1 text-[11px] font-medium uppercase tracking-wider text-t3">
            <PhaseIcon phase={phase.phase} className="size-3.5" />
            {PHASE_LABEL[phase.phase]}
          </h3>
          <ul className="flex list-none flex-col gap-1">
            {phase.groups.flatMap((group) =>
              group.attempts.map((attempt) => {
                const elapsed =
                  attempt.startedAt
                    ? Math.max(
                        0,
                        Math.round(
                          ((attempt.finishedAt ? Date.parse(attempt.finishedAt) : Date.now()) -
                            Date.parse(attempt.startedAt)) /
                            1000
                        )
                      )
                    : null;
                const selected = attempt.runId === selectedRunId;
                return (
                  <li key={attempt.runId}>
                    <button
                      type="button"
                      onClick={() => onSelect({ runId: attempt.runId, role: group.role, phase: phase.phase })}
                      aria-current={selected ? "true" : undefined}
                      className={cn(
                        // 每行 48–56 px，容得下 28 px 图标、名称和一行结果 (05 §5.3)。
                        "flex min-h-[52px] w-full items-center gap-2 rounded-md border px-2 py-1.5 text-left transition-colors",
                        selected
                          ? "border-line-strong bg-raised"
                          : "border-transparent hover:border-line hover:bg-raised/70",
                        !selected && attempt.state === "running" && "border-run/40"
                      )}
                    >
                      {attempt.agent ? (
                        <AgentMark kind={attempt.agent} size={28} />
                      ) : (
                        <span className="grid size-7 shrink-0 place-items-center rounded-lg border border-line bg-raised/70">
                          <PhaseIcon phase="validate" className="size-4" />
                        </span>
                      )}
                      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span className="flex min-w-0 items-center gap-1.5">
                          <span className="shrink-0 text-[12px] text-t3">{ROLE_LABEL[group.role]}</span>
                          <span className="truncate text-[13px]">
                            {attempt.agent ? agentLabel(attempt.agent) : "本机"}
                          </span>
                          {group.attempts.length > 1 && (
                            <span className="shrink-0 rounded-full border border-line px-1.5 text-[11px] text-t3">
                              {attempt.attemptLabel}
                            </span>
                          )}
                          {group.memberTotal != null && (
                            <span className="shrink-0 rounded-full border border-line px-1.5 text-[11px] text-t3">
                              成员 {group.memberIndex}/{group.memberTotal}
                            </span>
                          )}
                        </span>
                        <span className="flex min-w-0 items-center gap-1.5 text-[11px] text-t3">
                          {elapsed != null && <span className="tabular-nums">{formatElapsed(elapsed)}</span>}
                          {attempt.fallbackReason && <span className="truncate">· {attempt.fallbackReason}</span>}
                        </span>
                      </span>
                      <StateMark state={attempt.state} label={STATUS_LABEL[attempt.status]} />
                    </button>
                  </li>
                );
              })
            )}
          </ul>
        </section>
      ))}
    </div>
  );
}

/** 只有一个 run 时中间列表自动收起，把空间让给右侧内容 (05 §5.1)。 */
export function countRuns(tree: ExecutionTree, revision: number): number {
  const node = tree.revisions.find((r) => r.revision === revision);
  if (!node) return 0;
  return node.phases.reduce(
    (total, phase) => total + phase.groups.reduce((sum, group) => sum + group.attempts.length, 0),
    0
  );
}
