import type { Phase } from "@/copy/events";
import type { ExecutionTree } from "./tree";

/**
 * 执行树的键盘导航模型 (05 §8「树导航可用键盘完成」).
 *
 * 纯函数：把当前可见的树压平成有序列表，并把一次按键翻译成一个动作。
 * 这样方向键的语义可以脱离 DOM 测试，组件只负责执行结果。
 * 遵循 WAI-ARIA treeview：上下移动、右展开/进子级、左收起/回父级、Home/End 跳两端。
 */

export interface FlatNode {
  id: string;
  level: number;
  kind: "revision" | "phase" | "attempt";
  expandable: boolean;
  expanded: boolean;
  revision: number;
  phase: Phase | null;
  runId: string | null;
}

export const nodeId = {
  revision: (revision: number) => `rev:${revision}`,
  phase: (revision: number, phase: Phase) => `phase:${revision}:${phase}`,
  attempt: (runId: string) => `run:${runId}`,
};

export interface OpenState {
  isRevisionOpen: (revision: number) => boolean;
  isPhaseOpen: (revision: number, phase: Phase) => boolean;
}

/** 只包含当前**可见**的节点——折叠起来的子树不参与方向键移动。 */
export function flattenTree(tree: ExecutionTree, open: OpenState): FlatNode[] {
  const out: FlatNode[] = [];

  for (const revision of tree.revisions) {
    const revisionOpen = open.isRevisionOpen(revision.revision);
    out.push({
      id: nodeId.revision(revision.revision),
      level: 1,
      kind: "revision",
      expandable: revision.phases.length > 0,
      expanded: revisionOpen,
      revision: revision.revision,
      phase: null,
      runId: null,
    });
    if (!revisionOpen) continue;

    for (const phase of revision.phases) {
      const hasChildren = phase.groups.some((group) => group.attempts.length > 0);
      const phaseOpen = open.isPhaseOpen(revision.revision, phase.phase);
      out.push({
        id: nodeId.phase(revision.revision, phase.phase),
        level: 2,
        kind: "phase",
        expandable: hasChildren || phase.events.length > 0,
        expanded: phaseOpen,
        revision: revision.revision,
        phase: phase.phase,
        runId: null,
      });
      if (!phaseOpen) continue;

      for (const group of phase.groups) {
        for (const attempt of group.attempts) {
          out.push({
            id: nodeId.attempt(attempt.runId),
            level: 3,
            kind: "attempt",
            expandable: false,
            expanded: false,
            revision: revision.revision,
            phase: phase.phase,
            runId: attempt.runId,
          });
        }
      }
    }
  }

  return out;
}

export type NavAction =
  | { type: "focus"; id: string }
  | { type: "expand"; node: FlatNode }
  | { type: "collapse"; node: FlatNode }
  | { type: "select"; node: FlatNode };

/**
 * 把一次按键翻译成动作；返回 null 表示不拦截（交给浏览器默认行为，比如 Tab）。
 */
export function navigate(key: string, nodes: FlatNode[], activeId: string | null): NavAction | null {
  if (nodes.length === 0) return null;
  const index = Math.max(0, nodes.findIndex((node) => node.id === activeId));
  const node = nodes[index];

  switch (key) {
    case "ArrowDown":
      return { type: "focus", id: nodes[Math.min(index + 1, nodes.length - 1)].id };
    case "ArrowUp":
      return { type: "focus", id: nodes[Math.max(index - 1, 0)].id };
    case "Home":
      return { type: "focus", id: nodes[0].id };
    case "End":
      return { type: "focus", id: nodes[nodes.length - 1].id };
    case "ArrowRight":
      // 已展开时右键进入第一个子节点，符合 treeview 习惯。
      if (node.expandable && !node.expanded) return { type: "expand", node };
      if (node.expanded && nodes[index + 1]?.level > node.level) {
        return { type: "focus", id: nodes[index + 1].id };
      }
      return null;
    case "ArrowLeft": {
      if (node.expandable && node.expanded) return { type: "collapse", node };
      // 叶子节点上按左键回到父级，而不是原地不动。
      const parent = findParent(nodes, index);
      return parent ? { type: "focus", id: parent.id } : null;
    }
    case "Enter":
    case " ":
      return { type: "select", node };
    default:
      return null;
  }
}

function findParent(nodes: FlatNode[], index: number): FlatNode | null {
  const level = nodes[index].level;
  for (let i = index - 1; i >= 0; i -= 1) {
    if (nodes[i].level < level) return nodes[i];
  }
  return null;
}
