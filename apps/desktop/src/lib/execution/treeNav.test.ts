import { describe, expect, it } from "bun:test";
import type { RunSummary, TaskEvent } from "@/generated/bindings";
import { normalizeEvents } from "./normalize";
import { buildExecutionTree } from "./tree";
import { flattenTree, navigate, nodeId, type FlatNode, type OpenState } from "./treeNav";

const at = (min: number) => new Date(Date.UTC(2026, 0, 1, 12, min)).toISOString();

const run = (
  id: string,
  role: RunSummary["role"],
  agent: string | null,
  min: number,
  revision = 1
): RunSummary => ({
  id,
  taskId: "t1",
  revision,
  role,
  agent,
  status: "SUCCEEDED",
  exitCode: 0,
  costUsd: null,
  tokensIn: null,
  tokensOut: null,
  startedAt: at(min),
  finishedAt: at(min + 1),
});

const events: TaskEvent[] = [
  { id: 1, taskId: "t1", runId: null, revision: 1, actor: "agent", eventType: "run:succeeded", payload: {}, createdAt: at(2) },
  { id: 2, taskId: "t1", runId: null, revision: 2, actor: "agent", eventType: "run:succeeded", payload: {}, createdAt: at(9) },
];

const tree = buildExecutionTree({
  events: normalizeEvents(events),
  runs: [
    run("d1", "developer", "codex", 1, 1),
    run("d2", "developer", "codex", 8, 2),
    run("v2", "validator", null, 10, 2),
  ],
  currentRevision: 2,
  status: "REVIEWING",
  requirePlanApproval: false,
});

/** 默认：当前轮 r2 展开、其阶段展开；历史轮 r1 折叠。 */
const openAll: OpenState = { isRevisionOpen: (r) => r === 2, isPhaseOpen: () => true };

const ids = (nodes: FlatNode[]) => nodes.map((n) => n.id);

describe("压平可见树 (05 §8)", () => {
  it("折叠的 revision 不暴露子节点", () => {
    const flat = flattenTree(tree, openAll);

    expect(ids(flat)).toContain(nodeId.revision(1));
    // r1 折叠 → 它的阶段和 run 都不参与方向键移动。
    expect(ids(flat)).not.toContain(nodeId.phase(1, "develop"));
    expect(ids(flat)).not.toContain(nodeId.attempt("d1"));
  });

  it("折叠的阶段同样不暴露 run", () => {
    const flat = flattenTree(tree, { isRevisionOpen: () => true, isPhaseOpen: () => false });
    expect(ids(flat)).not.toContain(nodeId.attempt("d2"));
  });

  it("层级正确：revision 1 / 阶段 2 / run 3", () => {
    const flat = flattenTree(tree, openAll);
    const byId = new Map(flat.map((n) => [n.id, n]));

    expect(byId.get(nodeId.revision(2))!.level).toBe(1);
    expect(byId.get(nodeId.phase(2, "develop"))!.level).toBe(2);
    expect(byId.get(nodeId.attempt("d2"))!.level).toBe(3);
  });
});

describe("方向键语义 (WAI-ARIA treeview)", () => {
  const flat = flattenTree(tree, openAll);
  const first = flat[0].id;

  it("上下键在可见节点间移动，并在两端停住", () => {
    expect(navigate("ArrowDown", flat, first)).toEqual({ type: "focus", id: flat[1].id });
    expect(navigate("ArrowUp", flat, first)).toEqual({ type: "focus", id: first });
    expect(navigate("ArrowDown", flat, flat.at(-1)!.id)).toEqual({ type: "focus", id: flat.at(-1)!.id });
  });

  it("Home / End 跳到两端", () => {
    expect(navigate("Home", flat, flat[3].id)).toEqual({ type: "focus", id: flat[0].id });
    expect(navigate("End", flat, flat[0].id)).toEqual({ type: "focus", id: flat.at(-1)!.id });
  });

  it("右键先展开，已展开时进入第一个子节点", () => {
    const collapsed = flattenTree(tree, { isRevisionOpen: () => false, isPhaseOpen: () => false });
    const action = navigate("ArrowRight", collapsed, nodeId.revision(2));
    expect(action?.type).toBe("expand");

    const expanded = navigate("ArrowRight", flat, nodeId.revision(2));
    expect(expanded).toEqual({ type: "focus", id: flat[flat.findIndex((n) => n.id === nodeId.revision(2)) + 1].id });
  });

  it("左键先收起；叶子节点上回到父级", () => {
    expect(navigate("ArrowLeft", flat, nodeId.revision(2))?.type).toBe("collapse");
    // run 是叶子，左键应该回到它所属的阶段。
    expect(navigate("ArrowLeft", flat, nodeId.attempt("d2"))).toEqual({
      type: "focus",
      id: nodeId.phase(2, "develop"),
    });
  });

  it("Enter / 空格选中当前节点", () => {
    expect(navigate("Enter", flat, nodeId.attempt("d2"))?.type).toBe("select");
    expect(navigate(" ", flat, nodeId.attempt("d2"))?.type).toBe("select");
  });

  it("其它按键不拦截，Tab 仍然能离开树", () => {
    expect(navigate("Tab", flat, first)).toBeNull();
    expect(navigate("a", flat, first)).toBeNull();
  });

  it("焦点节点消失时从头开始，不会崩", () => {
    expect(navigate("ArrowDown", flat, "run:已经不存在了")).toEqual({ type: "focus", id: flat[1].id });
    expect(navigate("ArrowDown", [], null)).toBeNull();
  });
});
