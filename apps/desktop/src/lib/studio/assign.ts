import { SPOTS, spotById, type Spot, type SpotKind } from "./layout";
import type { Activity, MemberActivity } from "./activity";

export interface Placement {
  id: string;
  activity: Activity;
  taskCode: string | null;
  taskId: string | null;
  /** 分到的点位；工作点位都满了时为 null，并标记 overflow。 */
  spot: string | null;
  overflow: boolean;
}

const KIND_FOR: Record<Activity, SpotKind> = {
  plan: "plan",
  develop: "desk",
  validate: "validate",
  review: "review",
  waiting: "queue",
  idle: "rest",
};

/** 规划 / 审查 / 验证的专属点位满了时，退到空工位上做。 */
const FALLBACK: Partial<Record<Activity, SpotKind>> = { plan: "desk", review: "desk" };

function hash(id: string): number {
  let h = 2166136261;
  for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 16777619);
  return h >>> 0;
}

/**
 * 把成员分到点位。
 *
 * - 上一次的分配（previous）仍然合适就保留，机器人不会无故换座位。
 * - 工作类按点位数组顺序依次占用；空闲的按 id 稳定地挑休息点，满了顺延。
 * - 先排「等你」，再排工作中，最后排空闲，保证最重要的状态总有位置。
 */
export function assignSpots(members: readonly MemberActivity[], previous: ReadonlyMap<string, string> = new Map()): Placement[] {
  const taken = new Set<string>();
  const result = new Map<string, Placement>();
  const free = (kind: SpotKind): Spot[] => SPOTS.filter((spot) => spot.kind === kind && !taken.has(spot.id));

  const priority = (m: MemberActivity) => (m.activity === "waiting" ? 0 : m.activity === "idle" ? 2 : 1);
  const ordered = members.map((m, index) => ({ m, index })).sort((a, b) => priority(a.m) - priority(b.m) || a.index - b.index);

  // 先保留仍然合适的旧座位
  for (const { m } of ordered) {
    const prev = previous.get(m.id);
    const spot = prev ? spotById(prev) : undefined;
    const kinds = [KIND_FOR[m.activity], FALLBACK[m.activity]].filter(Boolean);
    if (spot && kinds.includes(spot.kind) && !taken.has(spot.id)) {
      taken.add(spot.id);
      result.set(m.id, { id: m.id, activity: m.activity, taskCode: m.taskCode, taskId: m.taskId, spot: spot.id, overflow: false });
    }
  }

  for (const { m } of ordered) {
    if (result.has(m.id)) continue;
    let spot: Spot | undefined;
    if (m.activity === "idle") {
      const rest = SPOTS.filter((s) => s.kind === "rest");
      const start = hash(m.id) % rest.length;
      for (let i = 0; i < rest.length && !spot; i++) {
        const candidate = rest[(start + i) % rest.length];
        if (!taken.has(candidate.id)) spot = candidate;
      }
    } else {
      spot = free(KIND_FOR[m.activity])[0];
      const fb = FALLBACK[m.activity];
      if (!spot && fb) spot = free(fb)[0];
    }
    if (spot) taken.add(spot.id);
    result.set(m.id, { id: m.id, activity: m.activity, taskCode: m.taskCode, taskId: m.taskId, spot: spot?.id ?? null, overflow: !spot });
  }

  return members.map((m) => result.get(m.id)!);
}
