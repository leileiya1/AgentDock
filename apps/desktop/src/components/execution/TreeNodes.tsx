import { ChevronRight } from "lucide-react";
import type { RunRole } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import type { NodeState } from "@/copy/events";
import type { AttemptNode, PhaseNode, RunGroupNode } from "@/lib/execution/tree";
import type { NormalizedEvent } from "@/lib/execution/normalize";
import { formatElapsed } from "@/lib/execution/liveStatus";
import { nodeId } from "@/lib/execution/treeNav";
import { AgentMark } from "@/components/AgentMark";
import { absoluteTime, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { PhaseIcon, StateMark } from "./StateMark";

/**
 * 节点组件层 (05 §10). Three independent visual channels per row (05 §3.2):
 * 谁在执行 = Provider 图标 + 名称；在做什么 = 阶段图标 + 中文阶段名；结果 = 状态图标 + 文字。
 */

export interface TreeSelection {
  revision: number;
  phase: PhaseNode["phase"] | null;
  runId: string | null;
}

const RUN_STATUS_LABEL: Record<AttemptNode["status"], string> = {
  RUNNING: "进行中",
  SUCCEEDED: "成功",
  FAILED: "失败",
  TIMED_OUT: "超时",
  CANCELLED: "已取消",
  INTERRUPTED: "已中断",
};

const ROLE_LABEL: Record<RunRole, string> = {
  planner: "计划",
  developer: "开发",
  reviewer: "审查",
  validator: "验证",
};

function elapsedOf(attempt: AttemptNode, now: number): number | null {
  if (!attempt.startedAt) return null;
  const end = attempt.finishedAt ? Date.parse(attempt.finishedAt) : now;
  return Math.max(0, Math.round((end - Date.parse(attempt.startedAt)) / 1000));
}

/** 当前节点可以轻微脉冲；全局 `prefers-reduced-motion` 已在 theme.css 里关闭动画。 */
const runningRing = (state: NodeState) => state === "running" && "ring-1 ring-status-running/40";

function EventLine({ event }: { event: NormalizedEvent }) {
  return (
    <li className="flex items-start gap-1.5 py-0.5 text-meta">
      <StateMark state={event.copy.state} iconOnly className="mt-px scale-90" />
      <span className="min-w-0 flex-1 text-t2">
        {event.copy.label}
        {event.copy.detail && <span className="mt-0.5 block text-meta text-t3">{event.copy.detail}</span>}
      </span>
    </li>
  );
}

function AttemptRow({
  attempt,
  showAttemptLabel,
  selected,
  onSelect,
  now,
  currentId,
  onFocusNode,
}: {
  attempt: AttemptNode;
  showAttemptLabel: boolean;
  selected: boolean;
  onSelect: () => void;
  now: number;
  currentId: string | null;
  onFocusNode: (id: string) => void;
}) {
  const elapsed = elapsedOf(attempt, now);
  const id = nodeId.attempt(attempt.runId);
  return (
    <li role="none">
      <button
        type="button"
        role="treeitem"
        aria-level={3}
        aria-selected={selected}
        data-tree-id={id}
        tabIndex={currentId === id ? 0 : -1}
        onFocus={() => onFocusNode(id)}
        onClick={onSelect}
        title={attempt.startedAt ? absoluteTime(attempt.startedAt) : undefined}
        className={cn(
          "flex w-full min-h-12 items-center gap-2 rounded-control px-2 py-1.5 text-left transition-colors hover:bg-raised",
          selected && "bg-raised ring-1 ring-line-strong",
          !selected && runningRing(attempt.state)
        )}
      >
        {attempt.agent ? (
          <AgentMark kind={attempt.agent} size={28} />
        ) : (
          // 验证节点不伪装成某个 AI Provider (05 §3.2)。
          <span className="grid size-7 shrink-0 place-items-center rounded-section border border-line bg-raised/70 text-t2">
            <PhaseIcon phase="validate" className="size-4" />
          </span>
        )}
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="truncate text-body">{attempt.agent ? agentLabel(attempt.agent) : "本机"}</span>
            {showAttemptLabel && (
              <span className="shrink-0 rounded-pill border border-line px-1.5 text-meta text-t3">
                {attempt.attemptLabel}
              </span>
            )}
          </span>
          {attempt.fallbackReason && (
            <span className="truncate text-meta text-t3">降级原因：{attempt.fallbackReason}</span>
          )}
        </span>
        <span className="flex shrink-0 flex-col items-end gap-0.5">
          <StateMark state={attempt.state} label={RUN_STATUS_LABEL[attempt.status]} />
          {elapsed != null && <span className="text-meta tabular-nums text-t3">{formatElapsed(elapsed)}</span>}
        </span>
      </button>

      {(attempt.events.length > 0 || attempt.recovery.length > 0) && (
        <ul className="ml-4 list-none border-l border-line/70 pl-3">
          {attempt.events.map((event) => (
            <EventLine key={event.id} event={event} />
          ))}
          {/* 恢复接管是当前 run 的子状态，不另起平级节点 (05 §6.8)。 */}
          {attempt.recovery.map((event) => (
            <EventLine key={event.id} event={event} />
          ))}
        </ul>
      )}
    </li>
  );
}

function RunGroupRow({
  group,
  selection,
  onSelect,
  now,
  currentId,
  onFocusNode,
}: {
  group: RunGroupNode;
  selection: TreeSelection | null;
  onSelect: (runId: string) => void;
  now: number;
  currentId: string | null;
  onFocusNode: (id: string) => void;
}) {
  const multiAttempt = group.attempts.length > 1;
  return (
    <li role="none">
      {group.memberTotal != null && (
        <div className="px-2 pt-1 text-meta text-t3">
          成员 {group.memberIndex}/{group.memberTotal}
        </div>
      )}
      <ul className="list-none" role="group">
        {group.attempts.map((attempt) => (
          <AttemptRow
            key={attempt.runId}
            attempt={attempt}
            showAttemptLabel={multiAttempt}
            selected={selection?.runId === attempt.runId}
            onSelect={() => onSelect(attempt.runId)}
            now={now}
            currentId={currentId}
            onFocusNode={onFocusNode}
          />
        ))}
      </ul>
    </li>
  );
}

export function PhaseRow({
  phase,
  revision,
  expanded,
  onToggle,
  selection,
  onSelectPhase,
  onSelectRun,
  now,
  currentId,
  onFocusNode,
}: {
  phase: PhaseNode;
  revision: number;
  expanded: boolean;
  onToggle: () => void;
  selection: TreeSelection | null;
  onSelectPhase: () => void;
  onSelectRun: (runId: string) => void;
  now: number;
  currentId: string | null;
  onFocusNode: (id: string) => void;
}) {
  const hasChildren = phase.groups.length > 0 || phase.events.length > 0;
  const selected = selection?.revision === revision && selection.phase === phase.phase && !selection.runId;
  const roles = [...new Set(phase.groups.map((g) => ROLE_LABEL[g.role]))];
  const id = nodeId.phase(revision, phase.phase);

  return (
    <li role="none">
      {/* 一个节点一个可聚焦元素：展开/收起交给方向键，Tab 不会在树里空转 (05 §8)。 */}
      <button
        type="button"
        role="treeitem"
        aria-level={2}
        aria-selected={selected}
        aria-expanded={hasChildren ? expanded : undefined}
        data-tree-id={id}
        tabIndex={currentId === id ? 0 : -1}
        onFocus={() => onFocusNode(id)}
        onClick={() => {
          onSelectPhase();
          if (hasChildren && !expanded) onToggle();
          else if (hasChildren && selected) onToggle();
        }}
        className={cn(
          "flex w-full items-center gap-1 rounded-control pr-2 text-left transition-colors hover:bg-raised",
          selected && "bg-raised ring-1 ring-line"
        )}
      >
        <span className="grid size-6 shrink-0 place-items-center text-t3" aria-hidden>
          {hasChildren && (
            <ChevronRight className={cn("size-3.5 transition-transform", expanded && "rotate-90")} />
          )}
        </span>
        <PhaseIcon phase={phase.phase} />
        <span className="min-w-0 flex-1 py-1.5">
          <span className="flex items-center gap-1.5">
            <span className="text-body font-medium">{phase.label}</span>
            {roles.length > 1 && <span className="text-meta text-t3">{roles.join(" · ")}</span>}
          </span>
          {phase.summary && <span className="mt-0.5 block truncate text-meta text-t3">{phase.summary}</span>}
        </span>
        <StateMark state={phase.state} />
      </button>

      {expanded && hasChildren && (
        <ul className="ml-3 list-none border-l border-line/70 pl-2" role="group">
          {phase.groups.map((group) => (
            <RunGroupRow
              key={group.key}
              group={group}
              selection={selection}
              onSelect={onSelectRun}
              now={now}
              currentId={currentId}
              onFocusNode={onFocusNode}
            />
          ))}
          {phase.events.length > 0 && (
            <li className="px-2 py-1">
              <ul className="list-none">
                {phase.events.map((event) => (
                  <EventLine key={event.id} event={event} />
                ))}
              </ul>
            </li>
          )}
        </ul>
      )}
    </li>
  );
}

/** 过去的 revision 默认折叠成一行结论 + 耗时 + 改动摘要 (05 §3.5)。 */
export function CollapsedRevisionRow({
  revision,
  conclusion,
  state,
  durationSecs,
  stat,
  onExpand,
  treeId,
  tabbable,
  onFocusNode,
}: {
  revision: number;
  conclusion: string;
  state: NodeState;
  durationSecs: number | null;
  stat: string | null;
  onExpand: () => void;
  treeId: string;
  tabbable: boolean;
  onFocusNode: () => void;
}) {
  return (
    <li role="none">
      <button
        type="button"
        role="treeitem"
        aria-level={1}
        aria-expanded={false}
        data-tree-id={treeId}
        tabIndex={tabbable ? 0 : -1}
        onFocus={onFocusNode}
        onClick={onExpand}
        className="flex w-full items-center gap-2 rounded-control px-2 py-2 text-left transition-colors hover:bg-raised"
      >
        <ChevronRight className="size-3.5 shrink-0 text-t3" aria-hidden />
        <span className="shrink-0 font-mono text-meta text-t2">r{revision}</span>
        <span className="min-w-0 flex-1 truncate text-meta text-t3">
          {conclusion}
          {stat && ` · ${stat}`}
        </span>
        {durationSecs != null && (
          <span className="shrink-0 text-meta tabular-nums text-t3">{formatElapsed(durationSecs)}</span>
        )}
        <StateMark state={state} iconOnly />
      </button>
    </li>
  );
}

export function RevisionHeader({
  revision,
  isCurrent,
  conclusion,
  state,
  startedAt,
  expanded,
  onToggle,
  treeId,
  tabbable,
  onFocusNode,
}: {
  revision: number;
  isCurrent: boolean;
  conclusion: string;
  state: NodeState;
  startedAt: string | null;
  expanded: boolean;
  onToggle: () => void;
  treeId: string;
  tabbable: boolean;
  onFocusNode: () => void;
}) {
  return (
    <button
      type="button"
      role="treeitem"
      aria-level={1}
      aria-expanded={expanded}
      data-tree-id={treeId}
      tabIndex={tabbable ? 0 : -1}
      onFocus={onFocusNode}
      onClick={onToggle}
      className="flex w-full items-center gap-2 rounded-control px-2 py-2 text-left transition-colors hover:bg-raised"
    >
      <ChevronRight className={cn("size-3.5 shrink-0 text-t3 transition-transform", expanded && "rotate-90")} aria-hidden />
      <span className="font-mono text-body font-semibold">r{revision}</span>
      <span className="text-meta text-t2">{conclusion}</span>
      {!isCurrent && startedAt && (
        <span className="ml-auto shrink-0 text-meta text-t3" title={absoluteTime(startedAt)}>
          {relativeTime(startedAt)}
        </span>
      )}
      <StateMark state={state} iconOnly className={cn(isCurrent && "ml-auto")} />
    </button>
  );
}
