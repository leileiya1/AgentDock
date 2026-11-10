import { spotById, type Pt } from "./layout";
import { route } from "./path";
import type { Placement } from "./assign";

/** 场景里的一个机器人：位置用地面格坐标（脚下中心点）。 */
export interface Actor {
  id: string;
  /** 目标点位；正在走时是要去的地方。 */
  spot: string;
  pos: Pt;
  /** 剩余要走的折线点（不含当前位置）。空 = 已到达。 */
  path: Pt[];
  /** 走路朝向：朝镜头（x 或 y 增大）为 front，背对为 back。 */
  facing: "front" | "back";
}

export const WALK_SPEED = 13; // 地面格 / 秒

/**
 * 根据最新分配更新场景里的机器人。
 * - 新出现的成员直接出现在点位上，不从门口走进来，免得刚打开页面就满屋乱跑。
 * - 换了点位的成员沿走道走过去（animate=false 时直接瞬移，例如减少动态效果）。
 * - 没有点位（overflow）的成员不在场景里画。
 */
export function syncActors(prev: ReadonlyMap<string, Actor>, placements: readonly Placement[], animate: boolean): Map<string, Actor> {
  const next = new Map<string, Actor>();
  for (const p of placements) {
    if (!p.spot) continue;
    const target = spotById(p.spot);
    if (!target) continue;
    const old = prev.get(p.id);
    if (!old) {
      next.set(p.id, { id: p.id, spot: p.spot, pos: target.at, path: [], facing: "front" });
      continue;
    }
    if (old.spot === p.spot) {
      next.set(p.id, old);
      continue;
    }
    if (!animate) {
      next.set(p.id, { ...old, spot: p.spot, pos: target.at, path: [] });
      continue;
    }
    // 半路又换了目的地：先走完当前这段，再从原目的地接新路线（不会穿家具抄近道）
    const leg = (route(old.spot, p.spot) ?? [spotById(old.spot)?.at ?? old.pos, target.at]).slice(1);
    next.set(p.id, { ...old, spot: p.spot, path: [...old.path, ...leg] });
  }
  return next;
}

/** 沿路线前进 dt 秒。到达后 path 为空。 */
export function stepActor(actor: Actor, dt: number, speed = WALK_SPEED): Actor {
  if (!actor.path.length) return actor;
  let move = speed * dt;
  let pos = actor.pos;
  const path = [...actor.path];
  let facing = actor.facing;
  while (move > 0 && path.length) {
    const t = path[0];
    const dx = t.x - pos.x, dy = t.y - pos.y;
    const d = Math.hypot(dx, dy);
    if (d > 1e-6) facing = dx + dy >= 0 ? "front" : "back";
    if (d <= move) {
      pos = t;
      move -= d;
      path.shift();
    } else {
      pos = { x: pos.x + (dx / d) * move, y: pos.y + (dy / d) * move };
      move = 0;
    }
  }
  return { ...actor, pos, path, facing };
}

export type PoseKey =
  | "stand-idle" | "stand-work" | "stand-hand" | "stand-cup" | "stand-rest"
  | "walk-front" | "walk-back" | "sit-front" | "sleep" | "sit-back" | "sit-side";

/** 贴图姿势：走路时看朝向；到达后看点位姿势和活动。 */
export function poseFor(actor: Actor, activity: Placement["activity"]): PoseKey {
  if (actor.path.length) return actor.facing === "front" ? "walk-front" : "walk-back";
  const spot = spotById(actor.spot);
  switch (spot?.pose) {
    case "sit-back":
      return "sit-back";
    case "sit-front":
      return "sit-front";
    case "sit-side":
      return "sit-side";
    case "sleep":
      return "sleep";
    default:
      if (activity === "waiting") return "stand-hand";
      if (activity === "idle") return spot?.id === "tea" ? "stand-cup" : "stand-rest";
      return "stand-work";
  }
}

/** 机器人在绘制顺序里的位置：到达时用点位深度，走路时用脚下坐标。 */
export function depthOf(actor: Actor): number {
  if (!actor.path.length) {
    const spot = spotById(actor.spot);
    if (spot) return spot.depth;
  }
  return actor.pos.x + actor.pos.y;
}

/** 站立 / 走路类贴图的锚点是机身左后角，需要从脚下中心点换算。 */
export function spriteOrigin(actor: Actor, pose: PoseKey): Pt {
  if (pose === "sit-back" || pose === "sit-side") return actor.pos;
  return { x: actor.pos.x - 4.25, y: actor.pos.y - 3 };
}
