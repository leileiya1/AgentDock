import type { AgentKind, TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";

/**
 * 首页「工作室小报」和「成就墙」：全部从真实任务状态算出来，不编造进度。
 * 合并时间用任务最后更新时间近似（合并后任务不再变化）。
 */

export type Weather = "sunny" | "cloudy" | "rainy" | "starry";

export interface Headline {
  text: string;
  taskId?: string;
  tone: "good" | "human" | "info";
}

export interface DailyPaper {
  weather: Weather;
  forecast: string;
  headlines: Headline[];
  star: { agent: AgentKind; merged: number } | null;
}

const DAY = 24 * 3600 * 1000;
const sameDay = (a: Date, b: Date) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
const merged = (tasks: readonly TaskSummary[]) => tasks.filter((t) => t.status === "MERGED");

function ago(ms: number): string {
  const m = Math.max(1, Math.round(ms / 60000));
  if (m < 60) return `${m} 分钟`;
  const h = Math.round(m / 60);
  return h < 24 ? `${h} 小时` : `${Math.round(h / 24)} 天`;
}

export function dailyPaper(tasks: readonly TaskSummary[], now = new Date(), failing = 0): DailyPaper {
  const waiting = tasks.filter((t) => groupForStatus(t.status) === "attention");
  const running = tasks.filter((t) => groupForStatus(t.status) === "active");
  const today = merged(tasks).filter((t) => sameDay(new Date(t.updatedAt), now));
  const night = now.getHours() >= 19 || now.getHours() < 6;

  const weather: Weather = failing > 0 ? "rainy" : waiting.length > 0 ? "cloudy" : night ? "starry" : "sunny";
  const forecast =
    weather === "rainy" ? `有 ${failing} 件测试没过，工作室下着小雨`
    : weather === "cloudy" ? `${waiting.length} 件事等你拍板，云层有点厚`
    : weather === "starry" ? "没有待办，夜空很干净"
    : "没有待办，万里晴空";

  const headlines: Headline[] = [];
  headlines.push(
    today.length > 0
      ? { text: `今天已合并 ${today.length} 个任务`, tone: "good" }
      : { text: running.length > 0 ? `今天还没有合并，${running.length} 件正在路上` : "今天还没有合并，派个活试试？", tone: "info" }
  );
  const oldest = [...waiting].sort((a, b) => a.updatedAt.localeCompare(b.updatedAt))[0];
  if (oldest) {
    headlines.push({ text: `TASK-${String(oldest.seq).padStart(3, "0")}「${oldest.title}」已经等你 ${ago(now.getTime() - Date.parse(oldest.updatedAt))}`, taskId: oldest.id, tone: "human" });
  }
  const latest = [...running].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))[0];
  if (latest) headlines.push({ text: `正在忙：「${latest.title}」`, taskId: latest.id, tone: "info" });

  const week = merged(tasks).filter((t) => now.getTime() - Date.parse(t.updatedAt) <= 7 * DAY);
  const counts = new Map<AgentKind, number>();
  for (const t of week) counts.set(t.developerAgent, (counts.get(t.developerAgent) ?? 0) + 1);
  const best = [...counts.entries()].sort((a, b) => b[1] - a[1])[0];

  return { weather, forecast, headlines: headlines.slice(0, 3), star: best ? { agent: best[0], merged: best[1] } : null };
}

export interface Badge {
  key: string;
  name: string;
  /** 怎么得到的 / 还差多少。 */
  hint: string;
  earned: boolean;
  /** 0..1，没拿到时的进度。 */
  progress: number;
}

/** 连续有合并的天数（今天或昨天结尾都算还在连）。 */
export function mergeStreak(tasks: readonly TaskSummary[], now = new Date()): number {
  const days = new Set(merged(tasks).map((t) => new Date(t.updatedAt).toDateString()));
  let streak = 0;
  const d = new Date(now);
  if (!days.has(d.toDateString())) d.setDate(d.getDate() - 1);
  while (days.has(d.toDateString())) {
    streak++;
    d.setDate(d.getDate() - 1);
  }
  return streak;
}

export function badges(tasks: readonly TaskSummary[], now = new Date()): Badge[] {
  const done = merged(tasks).length;
  const streak = mergeStreak(tasks, now);
  const waiting = tasks.filter((t) => groupForStatus(t.status) === "attention").length;
  const crew = new Set(tasks.filter((t) => groupForStatus(t.status) === "active").flatMap((t) => [t.developerAgent, t.reviewerAgent])).size;
  const owl = merged(tasks).some((t) => new Date(t.updatedAt).getHours() < 5);
  const pct = (n: number, goal: number) => Math.min(1, n / goal);
  return [
    { key: "first", name: "开张大吉", hint: done >= 1 ? "合并了第一个任务" : "合并第一个任务", earned: done >= 1, progress: pct(done, 1) },
    { key: "ten", name: "十全十美", hint: done >= 10 ? "累计合并 10 个任务" : `累计合并 ${done} / 10`, earned: done >= 10, progress: pct(done, 10) },
    { key: "streak", name: "三连发", hint: streak >= 3 ? `已经连续 ${streak} 天有合并` : `连续 3 天都有合并（现在 ${streak} 天）`, earned: streak >= 3, progress: pct(streak, 3) },
    { key: "inbox", name: "收件箱清零", hint: waiting === 0 ? "没有等你处理的事" : `还有 ${waiting} 件等你`, earned: waiting === 0 && tasks.length > 0, progress: waiting === 0 ? 1 : 0 },
    { key: "crew", name: "全员上阵", hint: crew >= 3 ? `${crew} 位 Agent 同时在忙` : `同时有 3 位 Agent 在忙（现在 ${crew} 位）`, earned: crew >= 3, progress: pct(crew, 3) },
    { key: "owl", name: "夜猫子", hint: owl ? "凌晨还合并过任务" : "在凌晨合并一个任务", earned: owl, progress: owl ? 1 : 0 },
  ];
}
