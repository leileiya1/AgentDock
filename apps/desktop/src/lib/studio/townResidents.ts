import type { AgentKind, ProjectSummary } from "@/generated/bindings";
import type { PoseKey } from "./motion";
import { townAgents } from "@/lib/studio/projectSummary";

/**
 * 每个项目门前院子里的机器人：数量就是这个项目分配的 Agent 数（后端汇总的 assignedAgents；没分配过时退回最近任务推导的 agents），
 * 不是固定人数。正在干活的站在门口，等你处理时第一位在邮箱边举手，其余在院子里溜达。
 */

export interface ProjectLook {
  /** 窗户亮着（有任务在跑或在等你）。 */
  lit: boolean;
  smoke: boolean;
  flag: boolean;
  cloud: boolean;
  leaves: boolean;
  /** 一个月没动：院子里长草。 */
  weeds: boolean;
  /** 没动的天数（落叶 / 长草时给门牌用）。 */
  idleDays: number;
  /** 还没有任何任务：先是工地。 */
  construction: boolean;
}

const DAY_MS = 24 * 3600 * 1000;
const QUIET_AFTER_MS = 3 * DAY_MS;
const WEEDS_AFTER_MS = 30 * DAY_MS;

export function projectLook(s: ProjectSummary | undefined, now: number): ProjectLook {
  if (!s || s.taskCount === 0) return { lit: false, smoke: false, flag: false, cloud: false, leaves: false, weeds: false, idleDays: 0, construction: true };
  const last = s.lastActivityAt ? Date.parse(s.lastActivityAt) : 0;
  const quiet = s.running === 0 && s.waitingForYou === 0 && now - last > QUIET_AFTER_MS;
  return {
    lit: s.running > 0 || s.waitingForYou > 0,
    smoke: s.running > 0,
    flag: s.waitingForYou > 0,
    cloud: s.failing > 0,
    leaves: quiet,
    weeds: quiet && now - last > WEEDS_AFTER_MS,
    idleDays: quiet ? Math.floor((now - last) / DAY_MS) : 0,
    construction: false,
  };
}

export interface Resident {
  agent: AgentKind;
  /** 院子里的位置（街区内坐标，脚下中心点）。 */
  x: number;
  y: number;
  pose: PoseKey;
}

/** 走一步需要的时间（秒）和每个溜达周期的长度。 */
const WALK_SECONDS = 2.4;
const PERIOD_SECONDS = 9;

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

/**
 * 某一时刻每个机器人在哪、什么姿势。纯函数：同样的输入和时间得到同样的画面，
 * reduce=true（减少动态效果）时不走动。
 */
export function residents(
  projectId: string,
  summary: ProjectSummary | undefined,
  door: { x: number; y: number },
  mailbox: { x: number; y: number },
  yard: ReadonlyArray<readonly [number, number]>,
  timeSeconds: number,
  reduce = false
): Resident[] {
  const agents = townAgents(summary);
  if (!summary || agents.length === 0 || yard.length === 0) return [];
  const active = new Set(summary.activeAgents);
  const out: Resident[] = [];
  let waiterPlaced = false;
  let doorSlot = 0;
  agents.forEach((agent, i) => {
    if (active.has(agent)) {
      out.push({ agent, x: door.x - 6 + (doorSlot++ % 3) * 6, y: door.y + 4, pose: "stand-work" });
      return;
    }
    if (summary.waitingForYou > 0 && !waiterPlaced) {
      waiterPlaced = true;
      out.push({ agent, x: mailbox.x - 3, y: mailbox.y + 4, pose: "stand-hand" });
      return;
    }
    const seed = hash(projectId + agent);
    const start = seed % yard.length;
    if (reduce) {
      const [x, y] = yard[(start + i) % yard.length];
      out.push({ agent, x, y, pose: "stand-rest" });
      return;
    }
    const phase = (seed % 1000) / 1000 * PERIOD_SECONDS;
    const k = Math.floor((timeSeconds + phase) / PERIOD_SECONDS);
    const into = (timeSeconds + phase) - k * PERIOD_SECONDS;
    const at = (n: number) => yard[(start + i + n * 3) % yard.length];
    const [tx, ty] = at(k);
    if (into < WALK_SECONDS) {
      const [fx, fy] = at(k - 1);
      const u = into / WALK_SECONDS;
      const x = fx + (tx - fx) * u, y = fy + (ty - fy) * u;
      const toward = tx - fx + (ty - fy) >= 0;
      out.push({ agent, x, y, pose: toward ? "walk-front" : "walk-back" });
    } else {
      out.push({ agent, x: tx, y: ty, pose: "stand-rest" });
    }
  });
  return out;
}
