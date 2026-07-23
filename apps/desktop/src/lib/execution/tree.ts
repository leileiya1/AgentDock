import type { AgentKind, RunRole, RunStatus, RunSummary, TaskStatus } from "@/generated/bindings";
import { PHASE_LABEL, PHASE_ORDER, type NodeState, type Phase } from "@/copy/events";
import { runHint, type NormalizedEvent } from "./normalize";

/**
 * 树模型层 (05 §10). Builds `revision → 业务阶段 → run` from normalized events plus the
 * run list. Pure and view-free: components only read this shape.
 *
 * Two rules the flat timeline got wrong and this model enforces (05 §3.1):
 *   • Provider 降级 is an *attempt under the original run*, never a sibling top-level node.
 *   • 委员会成员 are parallel branches of the same 审查阶段, not separate phases.
 */

export interface AttemptNode {
  runId: string;
  agent: AgentKind | null;
  status: RunStatus;
  state: NodeState;
  /** 首选 / 降级 1 / 降级 2 (05 §5.3). */
  attemptLabel: string;
  /** 为什么降级到这次尝试——只有降级尝试才有。 */
  fallbackReason: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  /** 挂在这次尝试下的事件（降级说明、结构修复…）。 */
  events: NormalizedEvent[];
  /** daemon 重连接管作为当前 run 的子状态，不另起平级节点 (05 §6.8)。 */
  recovery: NormalizedEvent[];
}

export interface RunGroupNode {
  key: string;
  role: RunRole;
  /** 委员会成员显示 `成员 1/3`；主执行单元不显示。 */
  memberIndex: number | null;
  memberTotal: number | null;
  attempts: AttemptNode[];
  /** 最新一次尝试，用于折叠态展示。 */
  current: AttemptNode;
  state: NodeState;
}

export interface PhaseNode {
  phase: Phase;
  label: string;
  state: NodeState;
  /** 阶段结论的一行摘要，点击阶段时展示。 */
  summary: string | null;
  groups: RunGroupNode[];
  /** 阶段级事件（不属于任何具体 run）。 */
  events: NormalizedEvent[];
  startedAt: string | null;
  endedAt: string | null;
}

export interface RevisionNode {
  revision: number;
  phases: PhaseNode[];
  state: NodeState;
  startedAt: string | null;
  endedAt: string | null;
  /** 折叠行的结论文字 (05 §3.5)。 */
  conclusion: string;
}

export interface ExecutionTree {
  /** 当前轮在最前，历史轮降序。 */
  revisions: RevisionNode[];
  /** scheduler slot、结果文件、心跳等，默认折叠 (05 §3.3)。 */
  systemEvents: NormalizedEvent[];
  currentRevision: number;
}

const ROLE_PHASE: Record<RunRole, Phase> = {
  planner: "plan",
  developer: "develop",
  reviewer: "review",
  validator: "validate",
};

const RUN_STATE: Record<RunStatus, NodeState> = {
  RUNNING: "running",
  SUCCEEDED: "ok",
  FAILED: "failed",
  TIMED_OUT: "failed",
  CANCELLED: "info",
  INTERRUPTED: "attention",
};

/** 终态优先级：失败 > 需要处理 > 进行中 > 通过 > 其它。 */
const STATE_RANK: Record<NodeState, number> = {
  failed: 5,
  attention: 4,
  running: 3,
  ok: 2,
  info: 1,
  pending: 0,
};

function worst(states: NodeState[]): NodeState {
  return states.reduce<NodeState>((acc, s) => (STATE_RANK[s] > STATE_RANK[acc] ? s : acc), "pending");
}

function byTime(a: string | null, b: string | null): number {
  return (a ? Date.parse(a) : 0) - (b ? Date.parse(b) : 0);
}

function byEventTime(a: NormalizedEvent, b: NormalizedEvent): number {
  return byTime(a.ts, b.ts) || a.id - b.id;
}

/**
 * Phase events are lifecycle transitions, not parallel health signals. Once a later
 * terminal event arrives (for example `merge:succeeded` after `human:merge`), it must
 * close the earlier running state. Run groups still win while a real process is active.
 */
function phaseState(groups: RunGroupNode[], events: NormalizedEvent[]): NodeState {
  if (groups.some((g) => g.state === "running")) return "running";
  const latestEvent = events.at(-1);
  if (latestEvent) return latestEvent.copy.state;
  return worst(groups.map((g) => g.state));
}

function settlePhaseEvents(events: NormalizedEvent[]): NormalizedEvent[] {
  const ordered = events.slice().sort(byEventTime);
  const latest = ordered.at(-1);
  if (!latest || latest.copy.state === "running" || latest.copy.state === "pending") return ordered;

  // A start/progress event remains useful history after completion, but it must no
  // longer render as an active spinner once a later terminal transition exists.
  return ordered.map((event, index) =>
    index < ordered.length - 1 && event.copy.state === "running"
      ? { ...event, copy: { ...event.copy, state: "info" } }
      : event
  );
}

/**
 * A revision's runs of one role are either a 降级链 (Provider A failed → B took over) or
 * parallel 委员会成员. `provider:fallback` events are the authority: a run whose agent is
 * some fallback's `to` continues that fallback's `from` group; anything else starts a new group.
 */
interface FallbackLink {
  from: string;
  to: string;
  reason: string | null;
  event: NormalizedEvent;
}

function fallbackLinks(events: NormalizedEvent[], revision: number, role: RunRole): FallbackLink[] {
  return events
    .filter((e) => e.copy.attach === "run" && e.revision === revision && e.eventType === "provider:fallback")
    .filter((e) => {
      const hint = runHint(e);
      return hint.role == null || hint.role === role;
    })
    .map((e) => ({
      from: String(e.payload?.from ?? ""),
      to: String(e.payload?.to ?? ""),
      reason: e.copy.detail ?? null,
      event: e,
    }))
    .filter((link) => link.from && link.to);
}

const ATTEMPT_LABEL = (index: number) => (index === 0 ? "首选" : `降级 ${index}`);

function buildGroups(
  revision: number,
  role: RunRole,
  runs: RunSummary[],
  events: NormalizedEvent[]
): RunGroupNode[] {
  const ordered = runs.slice().sort((a, b) => byTime(a.startedAt, b.startedAt));
  const links = fallbackLinks(events, revision, role);
  const consumed = new Set<FallbackLink>();

  const chains: RunSummary[][] = [];
  const chainByAgent = new Map<string, RunSummary[]>();

  for (const run of ordered) {
    const link = run.agent
      ? links.find((l) => !consumed.has(l) && l.to === run.agent && chainByAgent.has(l.from))
      : undefined;
    if (link) {
      consumed.add(link);
      const chain = chainByAgent.get(link.from)!;
      chain.push(run);
      if (run.agent) chainByAgent.set(run.agent, chain);
      continue;
    }
    const chain = [run];
    chains.push(chain);
    if (run.agent) chainByAgent.set(run.agent, chain);
  }

  const council = chains.length > 1;
  return chains.map((chain, chainIndex) => {
    const attempts = chain.map((run, attemptIndex) => {
      const incoming = links.find((l) => l.to === run.agent && attemptIndex > 0);
      const own = events.filter((e) => e.runId === run.id);
      return {
        runId: run.id,
        agent: run.agent,
        status: run.status,
        state: RUN_STATE[run.status],
        attemptLabel: ATTEMPT_LABEL(attemptIndex),
        fallbackReason: attemptIndex > 0 ? incoming?.reason ?? null : null,
        startedAt: run.startedAt,
        finishedAt: run.finishedAt,
        events: own.filter((e) => e.copy.attach === "run"),
        recovery: own.filter((e) => e.copy.attach === "recovery"),
      } satisfies AttemptNode;
    });
    // 降级说明要挂到「降级到的那次尝试」上，即使后端没有 run_id。
    for (const link of links) {
      const target = attempts.find((a) => a.agent === link.to);
      if (target && !target.events.includes(link.event)) target.events.push(link.event);
    }
    const current = attempts[attempts.length - 1];
    return {
      key: `${revision}-${role}-${chainIndex}`,
      role,
      memberIndex: council ? chainIndex + 1 : null,
      memberTotal: council ? chains.length : null,
      attempts,
      current,
      // 只有最后一次尝试代表这条链的结论；前面的失败已经解释为「降级原因」。
      state: current.state,
    } satisfies RunGroupNode;
  });
}

function phaseSummary(phase: Phase, events: NormalizedEvent[], groups: RunGroupNode[]): string | null {
  const withDetail = events.filter((e) => e.copy.detail);
  const last = withDetail[withDetail.length - 1];
  if (last?.copy.detail) return last.copy.detail;
  if (phase === "review" && groups.length > 1) {
    const done = groups.filter((g) => g.state !== "running" && g.state !== "pending").length;
    return `委员会 ${done}/${groups.length} 已返回`;
  }
  const lastEvent = events[events.length - 1];
  return lastEvent?.copy.label ?? null;
}

function buildPhase(
  phase: Phase,
  groups: RunGroupNode[],
  events: NormalizedEvent[]
): PhaseNode {
  const orderedEvents = settlePhaseEvents(events);
  const times = [
    ...groups.flatMap((g) => g.attempts.map((a) => a.startedAt)),
    ...orderedEvents.map((e) => e.ts),
  ].filter((t): t is string => !!t);
  const state = phaseState(groups, orderedEvents);
  const ends = [
    ...groups.flatMap((g) => g.attempts.map((a) => a.finishedAt)),
    ...(state !== "running" && state !== "pending" ? [orderedEvents.at(-1)?.ts] : []),
  ].filter((t): t is string => !!t);
  return {
    phase,
    label: PHASE_LABEL[phase],
    state,
    summary: phaseSummary(phase, orderedEvents, groups),
    groups,
    events: orderedEvents,
    startedAt: times.sort()[0] ?? null,
    endedAt: state === "running" || state === "pending" ? null : ends.sort().at(-1) ?? null,
  };
}

/** 尚未开始的阶段也要出现，用户才知道「还差什么」(05 §3.1)。 */
function pendingPhase(phase: Phase): PhaseNode {
  return {
    phase,
    label: PHASE_LABEL[phase],
    state: "pending",
    summary: "尚未开始",
    groups: [],
    events: [],
    startedAt: null,
    endedAt: null,
  };
}

const TERMINAL_STATUSES: TaskStatus[] = ["MERGED", "ROLLED_BACK", "CANCELLED"];

function revisionConclusion(node: Omit<RevisionNode, "conclusion">, isCurrent: boolean): string {
  if (isCurrent) return "当前轮";
  const review = node.phases.find((p) => p.phase === "review");
  if (review?.state === "attention") return "要求返工";
  if (review?.state === "failed") return "审查未通过";
  const delivery = node.phases.find((p) => p.phase === "delivery");
  if (delivery?.state === "ok") return "已交付";
  if (node.state === "failed") return "本轮失败";
  return "本轮已结束";
}

export interface BuildTreeInput {
  events: NormalizedEvent[];
  runs: RunSummary[];
  currentRevision: number;
  status: TaskStatus;
  /** 计划审批开启时，计划阶段才是这条流程的一部分。 */
  requirePlanApproval: boolean;
}

export function buildExecutionTree(input: BuildTreeInput): ExecutionTree {
  const { events, runs, currentRevision, status, requirePlanApproval } = input;
  const systemEvents = events.filter((e) => e.systemDetail);
  const treeEvents = events.filter((e) => !e.systemDetail || e.copy.attach !== "phase");

  const revisionNumbers = new Set<number>();
  for (const run of runs) revisionNumbers.add(run.revision);
  for (const event of treeEvents) if (event.revision != null && event.revision > 0) revisionNumbers.add(event.revision);
  revisionNumbers.add(Math.max(1, currentRevision));

  const runsByRevision = new Map<number, RunSummary[]>();
  for (const run of runs) {
    const list = runsByRevision.get(run.revision) ?? [];
    list.push(run);
    runsByRevision.set(run.revision, list);
  }

  const revisions = [...revisionNumbers]
    .sort((a, b) => b - a)
    .map((revision) => {
      const isCurrent = revision === Math.max(1, currentRevision);
      const revisionRuns = runsByRevision.get(revision) ?? [];
      const revisionEvents = treeEvents.filter((e) => e.revision === revision || (isCurrent && e.revision == null));

      const phases: PhaseNode[] = [];
      for (const phase of PHASE_ORDER) {
        const roles = (Object.keys(ROLE_PHASE) as RunRole[]).filter((role) => ROLE_PHASE[role] === phase);
        const groups = roles.flatMap((role) =>
          buildGroups(revision, role, revisionRuns.filter((r) => r.role === role), revisionEvents)
        );
        const phaseEvents = revisionEvents.filter((e) => e.copy.phase === phase && e.copy.attach === "phase");
        if (groups.length === 0 && phaseEvents.length === 0) continue;
        if (phase === "plan" && !requirePlanApproval && phaseEvents.length === 0) continue;
        phases.push(buildPhase(phase, groups, phaseEvents));
      }

      if (isCurrent && !TERMINAL_STATUSES.includes(status)) {
        const reached = new Set(phases.map((p) => p.phase));
        const lastIndex = Math.max(...phases.map((p) => PHASE_ORDER.indexOf(p.phase)), -1);
        for (const phase of ["approval", "delivery"] as Phase[]) {
          if (!reached.has(phase) && PHASE_ORDER.indexOf(phase) > lastIndex) phases.push(pendingPhase(phase));
        }
      }

      const starts = phases.map((p) => p.startedAt).filter((t): t is string => !!t).sort();
      const ends = phases.map((p) => p.endedAt).filter((t): t is string => !!t).sort();
      const base = {
        revision,
        phases,
        state: worst(phases.filter((p) => p.state !== "pending").map((p) => p.state)),
        startedAt: starts[0] ?? null,
        endedAt: phases.some((p) => p.state === "running" || p.state === "pending") ? null : ends.at(-1) ?? null,
      };
      return {
        ...base,
        conclusion: revisionConclusion(base, isCurrent && !TERMINAL_STATUSES.includes(status)),
      };
    });

  return { revisions, systemEvents, currentRevision: Math.max(1, currentRevision) };
}
