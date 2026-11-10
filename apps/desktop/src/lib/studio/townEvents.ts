import type { AgentKind, ProjectSummary } from "@/generated/bindings";
import { townAgents } from "@/lib/studio/projectSummary";

/** 小镇事件（纯逻辑）：工地施工队、一个月没动长草、工地变成新房。 */

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** 施工队：分配了 Agent 就是他们（最多 3 位），没分配就派两位默认的来盖房。 */
export function constructionCrew(summary: ProjectSummary | undefined): AgentKind[] {
  const agents = townAgents(summary);
  return (agents.length ? agents : (["claude_code", "codex"] as AgentKind[])).slice(0, 3);
}

/** 施工队站位（街区内坐标，脚下）：围着工地。 */
export const CREW_SPOTS: ReadonlyArray<readonly [number, number]> = [
  [74, 50],
  [60, 62],
  [80, 66],
];

/** 抡锤：每人错开节拍，每 0.8 秒一锤，锤下去那一下跳起 2px、迸出火星。 */
export function hammerBeat(time: number, index: number, reduce: boolean): { hop: number; spark: boolean } {
  if (reduce) return { hop: 0, spark: false };
  const t = ((((time + index * 0.27) % 0.8) + 0.8) % 0.8);
  return { hop: t < 0.12 ? 2 : 0, spark: t >= 0.12 && t < 0.24 };
}

export interface Weed {
  x: number;
  y: number;
  /** 草叶高度，4–8 像素。 */
  blades: number[];
  /** 摆动相位。 */
  phase: number;
  /** 顶上开一朵小黄花（蒲公英）。 */
  flower: boolean;
}

/** 院子里的杂草：每个项目位置固定（按 id 散列），不会每次刷新都换地方。 */
export function weedsFor(projectId: string, count = 16): Weed[] {
  let h = hash(projectId);
  const next = () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507) >>> 0;
    h ^= h >>> 13;
    return (h >>> 0) / 4294967296;
  };
  return Array.from({ length: count }, () => ({
    x: Math.round(18 + next() * 62),
    y: Math.round(56 + next() * 28),
    blades: Array.from({ length: 5 + Math.floor(next() * 4) }, () => 4 + Math.floor(next() * 5)),
    phase: next() * Math.PI * 2,
    flower: next() < 0.35,
  }));
}

/** 上一帧还是工地、这一帧已经不是了的项目 = 新房落成。 */
export function justBuilt(prev: ReadonlySet<string> | null, nowConstruction: ReadonlySet<string>, all: readonly string[]): string[] {
  if (!prev) return [];
  return all.filter((id) => prev.has(id) && !nowConstruction.has(id));
}
