import { describe, expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { EDGES, FURNITURE, NODES, ROOM, SPOTS, nodeById, segmentHitsFootprint } from "./layout";
import { nodePath, pathLength, route } from "./path";
import { memberActivities } from "./activity";
import { assignSpots } from "./assign";

function task(seq: number, status: TaskSummary["status"], dev = "claude_code", rev = "codex", updatedAt = "2026-11-06T00:00:00Z"): TaskSummary {
  return { id: `t${seq}`, projectId: "p", seq, title: `任务 ${seq}`, status, blockedReason: null, currentRevision: 1, developerAgent: dev, reviewerAgent: rev, updatedAt };
}

describe("布局数据", () => {
  test("节点、点位都在房间里，id 唯一", () => {
    const ids = [...NODES.map((n) => n.id), ...SPOTS.map((s) => s.id)];
    expect(new Set(NODES.map((n) => n.id)).size).toBe(NODES.length);
    expect(new Set(SPOTS.map((s) => s.id)).size).toBe(SPOTS.length);
    expect(ids.length).toBeGreaterThan(0);
    for (const p of [...NODES.map((n) => n.at), ...SPOTS.map((s) => s.at)]) {
      expect(p.x).toBeGreaterThanOrEqual(0);
      expect(p.x).toBeLessThanOrEqual(ROOM.x);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.y).toBeLessThanOrEqual(ROOM.y);
    }
  });

  test("走道节点不压在家具上，每条边都不穿过家具", () => {
    for (const node of NODES) {
      for (const f of FURNITURE) expect(segmentHitsFootprint(node.at, node.at, f), `${node.id} 压在 ${f.id} 上`).toBe(false);
    }
    for (const [a, b] of EDGES) {
      const pa = nodeById(a)!.at, pb = nodeById(b)!.at;
      for (const f of FURNITURE) expect(segmentHitsFootprint(pa, pb, f), `${a}→${b} 穿过 ${f.id}`).toBe(false);
    }
  });

  test("所有走道节点互相连通，每个点位的入口节点存在", () => {
    for (const node of NODES) expect(nodePath(NODES[0].id, node.id), node.id).not.toBeNull();
    for (const spot of SPOTS) expect(nodeById(spot.node), spot.id).toBeDefined();
  });
});

describe("寻路", () => {
  test("从沙发走到 3 号工位沿前方走道，不穿过沙发和茶几", () => {
    const pts = route("sofa-1", "desk-3")!;
    expect(pts[0]).toEqual({ x: 29, y: 82 });
    expect(pts.at(-1)).toEqual({ x: 76, y: 96 });
    expect(pts).toContainEqual({ x: 14, y: 103 });
    expect(pathLength(pts)).toBeGreaterThan(50);
  });

  test("任意两个点位之间都能走到", () => {
    for (const a of SPOTS) for (const b of SPOTS) expect(route(a.id, b.id), `${a.id}→${b.id}`).not.toBeNull();
  });

  test("同一个点位不动，未知点位返回 null", () => {
    expect(route("tea", "tea")).toEqual([{ x: 99.25, y: 92 }]);
    expect(route("tea", "nowhere")).toBeNull();
  });
});

describe("活动推导", () => {
  test("按后端状态找到正在干活的人，验证与合并归验证机", () => {
    const acts = memberActivities([task(1, "DEVELOPING"), task(2, "VALIDATING"), task(3, "REVIEWING", "qoder_cli", "codex")]);
    expect(acts.find((a) => a.id === "claude_code")).toEqual({ id: "claude_code", activity: "develop", taskCode: "TASK-001", taskId: "t1" });
    expect(acts.find((a) => a.id === "runner")?.activity).toBe("validate");
    expect(acts.find((a) => a.id === "codex")?.activity).toBe("review");
    expect(acts.find((a) => a.id === "qoder_cli")?.activity).toBe("idle");
  });

  test("等你处理优先于工作中；没有任务的成员是空闲", () => {
    const acts = memberActivities([task(1, "DEVELOPING", "kimi_api"), task(2, "BLOCKED", "kimi_api")]);
    expect(acts.find((a) => a.id === "kimi_api")).toEqual({ id: "kimi_api", activity: "waiting", taskCode: "TASK-002", taskId: "t2" });
    expect(memberActivities([task(3, "MERGED")]).every((a) => a.activity === "idle")).toBe(true);
  });

  test("成员顺序稳定：开发者、审查者、验证机", () => {
    expect(memberActivities([task(1, "DRAFT", "a", "b"), task(2, "DRAFT", "c", "a")]).map((a) => a.id)).toEqual(["a", "c", "b", "runner"]);
  });
});

describe("分配点位", () => {
  const m = (id: string, activity: "plan" | "develop" | "validate" | "review" | "waiting" | "idle") => ({ id, activity, taskCode: null, taskId: null });

  test("开发按顺序占工位，规划去白板，审查去会议桌，等你去门口", () => {
    const out = assignSpots([m("a", "develop"), m("b", "develop"), m("c", "plan"), m("d", "review"), m("e", "waiting"), m("runner", "validate")]);
    expect(out.map((p) => p.spot)).toEqual(["desk-1", "desk-2", "whiteboard", "review-seat", "queue-1", "racks"]);
    expect(out.every((p) => !p.overflow)).toBe(true);
  });

  test("工位满了标记 overflow，不在场景里硬塞", () => {
    const out = assignSpots([m("a", "develop"), m("b", "develop"), m("c", "develop"), m("d", "develop")]);
    expect(out[3]).toMatchObject({ spot: null, overflow: true });
  });

  test("规划位被占时退到空工位", () => {
    const out = assignSpots([m("a", "plan"), m("b", "plan")]);
    expect(out.map((p) => p.spot)).toEqual(["whiteboard", "desk-1"]);
  });

  test("保留仍然合适的旧座位，机器人不乱换", () => {
    const prev = new Map([["b", "desk-3"]]);
    const out = assignSpots([m("a", "develop"), m("b", "develop")], prev);
    expect(out.map((p) => p.spot)).toEqual(["desk-1", "desk-3"]);
  });

  test("空闲成员稳定地去同一个休息点，且不重叠", () => {
    const members = ["grok_cli", "qwen_code", "minimax_api", "gemini_cli", "deepseek_api"].map((id) => m(id, "idle"));
    const a = assignSpots(members), b = assignSpots(members);
    expect(a).toEqual(b);
    const spots = a.map((p) => p.spot);
    expect(new Set(spots).size).toBe(spots.length);
    expect(spots.every((id) => SPOTS.find((s) => s.id === id)?.kind === "rest")).toBe(true);
  });

  test("等你的人先排，即使排在名单后面", () => {
    const out = assignSpots([m("a", "waiting"), m("b", "waiting"), m("c", "waiting"), m("d", "waiting")]);
    expect(out.filter((p) => p.overflow).map((p) => p.id)).toEqual(["d"]);
  });
});
