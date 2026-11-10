import type { AgentKind, Project, TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";

const DAY = 86_400_000;
export type Grade = "A+" | "A" | "B" | "C";

export interface WeeklyReport {
  from: Date;
  to: Date;
  merged: number;
  mergedPrev: number;
  /** 本周合并里返工过（不止一轮）的比例，0–1；没合并时为 null。 */
  reworkRate: number | null;
  waiting: number;
  trouble: number;
  mvp: { agent: AgentKind; merged: number } | null;
  busiest: { name: string; merged: number } | null;
  grade: Grade;
  comment: string;
}

const COMMENT: Record<Grade, string> = {
  "A+": "又快又稳，整个小镇都在为你们放烟花",
  A: "干得漂亮，继续保持这个节奏",
  B: "稳步前进，返工和卡点再少一点就更好了",
  C: "这周有点安静，下周派点活让大家动起来",
};

/** 按合并数、返工率和出岔子的数量打分。 */
export function gradeFor(merged: number, reworkRate: number | null, trouble: number): Grade {
  if (merged === 0) return "C";
  const rework = reworkRate ?? 0;
  if (merged >= 5 && rework <= 0.2 && trouble === 0) return "A+";
  if (merged >= 2 && rework <= 0.4 && trouble <= 1) return "A";
  if (rework <= 0.6 && trouble <= 3) return "B";
  return "C";
}

/** 最近 7 天（跨项目）的成绩单；上周 = 再往前 7 天。合并时间取任务最后更新时间。 */
export function weeklyReport(tasks: readonly TaskSummary[], projects: readonly Project[], now: Date): WeeklyReport {
  const to = now.getTime();
  const from = to - 7 * DAY;
  const inWeek = (t: TaskSummary) => {
    const at = Date.parse(t.updatedAt);
    return at > from && at <= to;
  };
  const merged = tasks.filter((t) => t.status === "MERGED" && inWeek(t));
  const mergedPrev = tasks.filter((t) => {
    const at = Date.parse(t.updatedAt);
    return t.status === "MERGED" && at > from - 7 * DAY && at <= from;
  }).length;
  const reworked = merged.filter((t) => t.currentRevision > 1).length;
  const trouble = tasks.filter((t) => (t.status === "MERGE_CONFLICT" || t.status === "ROLLED_BACK") && inWeek(t)).length;
  const waiting = tasks.filter((t) => groupForStatus(t.status) === "attention").length;

  const tally = <K extends string>(keys: K[]) => {
    const m = new Map<K, number>();
    for (const k of keys) m.set(k, (m.get(k) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1])[0];
  };
  const topAgent = tally(merged.map((t) => t.developerAgent));
  const topProject = tally(merged.map((t) => t.projectId));
  const nameOf = new Map(projects.map((p) => [p.id, p.name]));
  const reworkRate = merged.length ? reworked / merged.length : null;
  const grade = gradeFor(merged.length, reworkRate, trouble);
  return {
    from: new Date(from),
    to: now,
    merged: merged.length,
    mergedPrev,
    reworkRate,
    waiting,
    trouble,
    mvp: topAgent ? { agent: topAgent[0], merged: topAgent[1] } : null,
    busiest: topProject ? { name: nameOf.get(topProject[0]) ?? "", merged: topProject[1] } : null,
    grade,
    comment: grade === "C" && merged.length > 0 ? (trouble > 3 ? "冲突和回滚有点多，合并前多同步一下主干" : "返工有点多，验收标准写细一点会更顺") : COMMENT[grade],
  };
}

/** 「比上周 +3」这种小字。 */
export function trendText(now: number, prev: number): string {
  if (now === prev) return "和上周持平";
  return now > prev ? `比上周多 ${now - prev}` : `比上周少 ${prev - now}`;
}
