import { describe, expect, it } from "bun:test";
import type { RunSummary, TaskEvent } from "@/generated/bindings";
import { normalizeEvents } from "./normalize";
import { buildExecutionTree } from "./tree";
import { liveStatus, formatElapsed } from "./liveStatus";

let seq = 0;
const at = (min: number) => new Date(Date.UTC(2026, 0, 1, 12, min)).toISOString();

function event(
  eventType: string,
  opts: Partial<Pick<TaskEvent, "revision" | "runId" | "actor" | "payload">> & { min?: number } = {}
): TaskEvent {
  return {
    id: ++seq,
    taskId: "t1",
    runId: opts.runId ?? null,
    revision: opts.revision ?? 1,
    actor: opts.actor ?? "orchestrator",
    eventType,
    payload: opts.payload ?? {},
    createdAt: at(opts.min ?? 0),
  };
}

function run(
  id: string,
  role: RunSummary["role"],
  agent: string | null,
  status: RunSummary["status"],
  min: number,
  revision = 1
): RunSummary {
  return {
    id,
    taskId: "t1",
    revision,
    role,
    agent,
    status,
    exitCode: status === "SUCCEEDED" ? 0 : null,
    costUsd: null,
    tokensIn: null,
    tokensOut: null,
    startedAt: at(min),
    finishedAt: status === "RUNNING" ? null : at(min + 2),
  };
}

const build = (events: TaskEvent[], runs: RunSummary[], over: Partial<Parameters<typeof buildExecutionTree>[0]> = {}) =>
  buildExecutionTree({
    events: normalizeEvents(events),
    runs,
    currentRevision: 1,
    status: "DEVELOPING",
    requirePlanApproval: false,
    ...over,
  });

describe("执行树 · 系统事件隔离 (05 §3.3)", () => {
  it("scheduler slot 与结果文件事件不进入主树", () => {
    const tree = build(
      [event("scheduler:slot"), event("result:repair_started"), event("run:succeeded", { min: 3 })],
      [run("r1", "developer", "codex", "SUCCEEDED", 1)]
    );

    expect(tree.systemEvents.map((e) => e.eventType)).toEqual(["scheduler:slot"]);
    const labels = tree.revisions[0].phases.flatMap((p) => p.events.map((e) => e.copy.label));
    expect(labels).not.toContain("scheduler:slot");
    expect(labels).toContain("开发完成");
  });

  it("系统详情也不暴露原始事件名", () => {
    const tree = build([event("scheduler:global_budget_exhausted")], []);
    expect(tree.systemEvents[0].copy.label).toBe("调度 · global budget exhausted");
  });
});

describe("执行树 · Provider 降级 (05 §3.1 / §5.3)", () => {
  const events = [
    event("provider:fallback", { min: 3, payload: { role: "developer", from: "codex", to: "claude_code", reason: "额度不足" } }),
    event("provider:fallback", { min: 6, payload: { role: "developer", from: "claude_code", to: "deepseek_api", reason: "登录已过期" } }),
  ];
  const runs = [
    run("r1", "developer", "codex", "FAILED", 1),
    run("r2", "developer", "claude_code", "FAILED", 4),
    run("r3", "developer", "deepseek_api", "SUCCEEDED", 7),
  ];

  it("两次降级折叠为同一 run 的三次尝试，而不是三个顶层节点", () => {
    const develop = build(events, runs).revisions[0].phases.find((p) => p.phase === "develop")!;

    expect(develop.groups).toHaveLength(1);
    expect(develop.groups[0].attempts.map((a) => a.attemptLabel)).toEqual(["首选", "降级 1", "降级 2"]);
    expect(develop.groups[0].current.agent).toBe("deepseek_api");
    expect(develop.state).toBe("ok");
  });

  it("降级尝试说明「为什么降级」和「降级到谁」", () => {
    const develop = build(events, runs).revisions[0].phases.find((p) => p.phase === "develop")!;
    const [, second, third] = develop.groups[0].attempts;

    expect(second.fallbackReason).toBe("额度不足");
    expect(third.fallbackReason).toBe("登录已过期");
    expect(second.events[0].copy.label).toBe("Codex 不可用，已降级到 Claude Code");
  });
});

describe("执行树 · 结构化结果修复", () => {
  it("同一开发 Provider 的结果修复是串行尝试，不伪装成审查委员会", () => {
    const repairTree = build(
      [event("result:repair_started", { min: 3, payload: { role: "developer", agent: "qoder_cli" } })],
      [
        run("dev1", "developer", "qoder_cli", "FAILED", 1),
        run("dev2", "developer", "qoder_cli", "RUNNING", 4),
      ]
    );
    const develop = repairTree.revisions[0].phases.find((phase) => phase.phase === "develop")!;

    expect(develop.groups).toHaveLength(1);
    expect(develop.groups[0].attempts.map((attempt) => attempt.attemptLabel)).toEqual([
      "首选",
      "结构修复 1",
    ]);
    expect(develop.groups[0].memberIndex).toBeNull();

    const status = liveStatus({
      tree: repairTree,
      status: "DEVELOPING",
      lastActivityAt: at(4),
      now: Date.parse(at(4)),
    })!;
    expect(status.headline).toBe("Qoder CLI 正在开发");
    expect(status.detail).toBe("结构修复 1");
  });

  it("同一审查 Provider 的结果修复也不是两个委员会成员", () => {
    const repairTree = build(
      [event("result:repair_started", { min: 3, payload: { role: "reviewer", agent: "grok_cli" } })],
      [
        run("review1", "reviewer", "grok_cli", "FAILED", 1),
        run("review2", "reviewer", "grok_cli", "RUNNING", 4),
      ],
      { status: "REVIEWING" }
    );
    const review = repairTree.revisions[0].phases.find((phase) => phase.phase === "review")!;

    expect(review.groups).toHaveLength(1);
    expect(review.groups[0].attempts.map((attempt) => attempt.attemptLabel)).toEqual([
      "首选",
      "结构修复 1",
    ]);
    expect(liveStatus({
      tree: repairTree,
      status: "REVIEWING",
      lastActivityAt: at(4),
      now: Date.parse(at(4)),
    })!.headline).toBe("Grok CLI 正在审查");
  });
});

describe("执行树 · 审查委员会 (05 §6.11)", () => {
  const runs = [
    run("rv1", "reviewer", "claude_code", "SUCCEEDED", 10),
    run("rv2", "reviewer", "deepseek_api", "SUCCEEDED", 10),
    run("rv3", "reviewer", "openai_api", "FAILED", 10),
  ];
  const events = [
    event("review:council_member_failed", { min: 12, runId: "rv3", payload: { agent: "openai_api", error: "429" } }),
  ];

  it("三名成员是同一审查阶段下的并行分支", () => {
    const review = build(events, runs, { status: "REVIEWING" }).revisions[0].phases.find((p) => p.phase === "review")!;

    expect(review.groups).toHaveLength(3);
    expect(review.groups.map((g) => `${g.memberIndex}/${g.memberTotal}`)).toEqual(["1/3", "2/3", "3/3"]);
    // 成员是并行分支，不是彼此的降级尝试。
    expect(review.groups.map((g) => g.attempts.length)).toEqual([1, 1, 1]);
    expect(review.summary).toBe("委员会 3/3 已返回");
  });

  it("Provider 故障说明自己不是反对票", () => {
    const review = build(events, runs, { status: "REVIEWING" }).revisions[0].phases.find((p) => p.phase === "review")!;
    const failed = review.groups.find((g) => g.current.agent === "openai_api")!;

    expect(failed.current.events[0].copy.label).toContain("不计为反对票");
  });
});

describe("执行树 · daemon 接管 (05 §6.8)", () => {
  it("接管过程是当前 run 的子状态，不产生平级恢复节点", () => {
    const events = [
      event("recovery:run_adopted", { min: 2, runId: "r1", actor: "system", payload: { run_id: "r1" } }),
      event("recovery:run_recovered", { min: 3, runId: "r1", actor: "system", payload: { run_id: "r1" } }),
    ];
    const tree = build(events, [run("r1", "developer", "codex", "RUNNING", 1)]);
    const develop = tree.revisions[0].phases.find((p) => p.phase === "develop")!;

    expect(develop.groups[0].current.recovery.map((e) => e.copy.label)).toEqual([
      "正在确认这个 Agent 是否仍在运行",
      "已重新接管，日志继续同步",
    ]);
    expect(develop.events).toHaveLength(0);
    expect(tree.systemEvents).toHaveLength(0);
  });

  it("接管失败区分具体原因，而不是直接标成失败", () => {
    const events = [
      event("recovery:run_adoption_failed", {
        min: 3,
        runId: "r1",
        actor: "system",
        payload: { detail: "review result identity mismatch" },
      }),
    ];
    const tree = build(events, [run("r1", "reviewer", "codex", "INTERRUPTED", 1)], { status: "REVIEWING" });
    const recovery = tree.revisions[0].phases.find((p) => p.phase === "review")!.groups[0].current.recovery[0];

    expect(recovery.copy.detail).toBe("结果与本任务不匹配，已丢弃，需要重跑这一步");
  });
});

describe("执行树 · 轮次结构 (05 §3.1 / §3.5)", () => {
  it("历史轮降序排列并带结论", () => {
    const events = [
      event("run:succeeded", { revision: 1, min: 2 }),
      event("review:request_changes", { revision: 1, min: 4 }),
      event("run:succeeded", { revision: 2, min: 8 }),
    ];
    const runs = [
      run("r1", "developer", "codex", "SUCCEEDED", 1, 1),
      run("r2", "developer", "codex", "SUCCEEDED", 7, 2),
    ];
    const tree = build(events, runs, { currentRevision: 2 });

    expect(tree.revisions.map((r) => r.revision)).toEqual([2, 1]);
    expect(tree.revisions[0].conclusion).toBe("当前轮");
    expect(tree.revisions[1].conclusion).toBe("要求返工");
  });

  it("当前轮补齐尚未开始的阶段，用户才知道还差什么", () => {
    const tree = build([event("run:succeeded", { min: 2 })], [run("r1", "developer", "codex", "SUCCEEDED", 1)]);
    const pending = tree.revisions[0].phases.filter((p) => p.state === "pending");

    expect(pending.map((p) => p.label)).toEqual(["人工批准", "交付"]);
    expect(pending[0].summary).toBe("尚未开始");
  });

  it("合并成功终结开始合并状态，并停止当前轮", () => {
    const tree = build(
      [event("human:merge", { min: 8 }), event("merge:succeeded", { min: 9 })],
      [],
      { status: "MERGED" }
    );
    const revision = tree.revisions[0];
    const delivery = revision.phases.find((p) => p.phase === "delivery")!;

    expect(delivery.state).toBe("ok");
    expect(delivery.summary).toBe("已合并到目标分支");
    expect(delivery.events.map((e) => e.copy.state)).toEqual(["info", "ok"]);
    expect(delivery.endedAt).toBe(at(9));
    expect(revision.state).toBe("ok");
    expect(revision.endedAt).toBe(at(9));
    expect(revision.conclusion).toBe("已交付");
    expect(tree.revisions[0].phases.some((p) => p.state === "pending")).toBe(false);
  });

  it("任务进入阻断后，延迟到达的 RUNNING 快照不会继续转", () => {
    const tree = build(
      [event("run:started", { min: 1, runId: "r1" })],
      [run("r1", "developer", "codex", "RUNNING", 1)],
      { status: "BLOCKED" }
    );
    const develop = tree.revisions[0].phases.find((p) => p.phase === "develop")!;

    expect(develop.state).toBe("info");
    expect(develop.groups[0].state).toBe("info");
    expect(develop.groups[0].current.state).toBe("info");
    expect(develop.events.every((item) => item.copy.state !== "running")).toBe(true);
  });

  it("并行分支已有失败时，也会关闭另一分支残留的 RUNNING", () => {
    const tree = build(
      [],
      [
        run("r1", "reviewer", "codex", "FAILED", 1),
        run("r2", "reviewer", "qoder_cli", "RUNNING", 1),
      ],
      { status: "BLOCKED" }
    );
    const review = tree.revisions[0].phases.find((p) => p.phase === "review")!;

    expect(review.state).toBe("failed");
    expect(review.groups.some((group) => group.state === "running")).toBe(false);
    expect(review.groups.flatMap((group) => group.attempts).some((attempt) => attempt.state === "running")).toBe(false);
  });

  it("已合并任务会关闭残留的交付动画", () => {
    const tree = build([event("human:merge", { min: 8 })], [], { status: "MERGED" });
    const delivery = tree.revisions[0].phases.find((p) => p.phase === "delivery")!;

    expect(delivery.state).toBe("ok");
    expect(delivery.summary).toBe("已合并到目标分支");
    expect(delivery.events[0].copy.state).toBe("info");
    expect(tree.revisions[0].state).toBe("ok");
  });

  it("后到的计划批准关闭先前的等待状态", () => {
    const tree = build(
      [event("plan:proposed", { min: 1 }), event("human:plan_approve", { min: 2 })],
      [],
      { status: "READY_FOR_DEVELOPMENT", requirePlanApproval: true }
    );
    const plan = tree.revisions[0].phases.find((p) => p.phase === "plan")!;

    expect(plan.state).toBe("ok");
    expect(plan.endedAt).toBe(at(2));
  });
});

describe("持续运行状态 (05 §3.4)", () => {
  const tree = build([], [run("r1", "developer", "codex", "RUNNING", 0)]);

  it("给出「谁 · 在做什么 · 已用时」", () => {
    const status = liveStatus({
      tree,
      status: "DEVELOPING",
      lastActivityAt: at(2),
      now: Date.parse(at(2)) + 5_000,
    })!;

    expect(status.headline).toBe("Codex 正在开发");
    expect(formatElapsed(status.elapsedSecs)).toBe("02:05");
    expect(status.stalled).toBe(false);
  });

  it("超过 10 秒没有新活动时标记为「仍在运行」而不是静止", () => {
    const status = liveStatus({
      tree,
      status: "DEVELOPING",
      lastActivityAt: at(2),
      now: Date.parse(at(2)) + 30_000,
    })!;

    expect(status.stalled).toBe(true);
    expect(status.tone).toBe("running");
  });

  it("委员会进行中显示已返回成员数", () => {
    const councilTree = build([], [
      run("rv1", "reviewer", "claude_code", "SUCCEEDED", 1),
      run("rv2", "reviewer", "deepseek_api", "RUNNING", 1),
      run("rv3", "reviewer", "openai_api", "RUNNING", 1),
    ]);
    const status = liveStatus({ tree: councilTree, status: "REVIEWING", lastActivityAt: at(1), now: Date.parse(at(1)) })!;

    expect(status.headline).toBe("审查委员会进行中");
    expect(status.detail).toBe("1/3 成员已返回");
  });

  it("等待用户时说明任务为什么停住", () => {
    const status = liveStatus({
      tree: build([], []),
      status: "WAITING_FOR_HUMAN_APPROVAL",
      lastActivityAt: at(1),
    })!;

    expect(status.tone).toBe("attention");
    expect(status.headline).toBe("等待你确认本轮改动");
  });

  it("任务已合并时，即使事件快照仍是开始合并也不显示运行状态", () => {
    const staleTree = build([event("human:merge", { min: 8 })], [], { status: "MERGING" });

    expect(liveStatus({
      tree: staleTree,
      status: "MERGED",
      lastActivityAt: at(8),
      now: Date.parse(at(8)) + 60_000,
    })).toBeNull();
  });

  it("任务已阻断时，即使 run 快照仍为 RUNNING 也先显示检查点状态", () => {
    const staleTree = build([], [run("r1", "developer", "codex", "RUNNING", 0)]);
    const status = liveStatus({
      tree: staleTree,
      status: "BLOCKED",
      lastActivityAt: at(1),
      now: Date.parse(at(1)) + 60_000,
    });

    expect(status?.headline).toBe("需要你处理");
    expect(status?.detail).toBe("任务已安全停在检查点");
    expect(status?.tone).toBe("attention");
  });
});
