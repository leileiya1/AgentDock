import type { AgentKind, ProjectSummary } from "@/generated/bindings";

/** 侧栏每个项目下面那一行小字和右边的角标。 */
export interface ProjectStatus {
  /** 一句话状态：「2 件在跑」「很久没动」「刚搬来」…… */
  line: string;
  tone: "human" | "running" | "danger" | "quiet" | "new";
  /** 等你处理的件数（角标）。 */
  waiting: number;
  running: boolean;
}

const DAY = 24 * 3600 * 1000;

export function projectStatus(s: ProjectSummary | undefined, now = Date.now()): ProjectStatus {
  if (!s || s.taskCount === 0) return { line: "刚搬来，还没派活", tone: "new", waiting: 0, running: false };
  const running = s.running > 0;
  if (s.failing > 0) return { line: `${s.failing} 件测试没过`, tone: "danger", waiting: s.waitingForYou, running };
  if (s.waitingForYou > 0) return { line: `${s.waitingForYou} 件等你拍板`, tone: "human", waiting: s.waitingForYou, running };
  if (running) return { line: `${s.running} 件在跑`, tone: "running", waiting: 0, running };
  const last = s.lastActivityAt ? Date.parse(s.lastActivityAt) : NaN;
  if (Number.isFinite(last) && now - last > 3 * DAY) return { line: `${Math.floor((now - last) / DAY)} 天没动了`, tone: "quiet", waiting: 0, running };
  return { line: `${s.done} 件已完成`, tone: "quiet", waiting: 0, running };
}

/** 此刻在岗（正在干活）的 Agent，跨项目去重，最多 limit 位。 */
export function onDuty(summaries: readonly ProjectSummary[] | undefined, limit = 6): AgentKind[] {
  const out: AgentKind[] = [];
  for (const s of summaries ?? []) for (const a of s.activeAgents) if (!out.includes(a)) out.push(a);
  return out.slice(0, limit);
}

/** 顶部问候：按当地时间，配一个太阳 / 月亮。 */
export function greeting(now = new Date()): { text: string; night: boolean } {
  const h = now.getHours();
  if (h < 5) return { text: "夜深了，老板", night: true };
  if (h < 11) return { text: "早上好，老板", night: false };
  if (h < 14) return { text: "中午好，老板", night: false };
  if (h < 19) return { text: "下午好，老板", night: false };
  return { text: "晚上好，老板", night: true };
}

export const SORT_LABEL = { default: "默认", waiting: "等我的优先", recent: "最近活跃" } as const;

interface Sortable {
  id: string;
  waiting: number;
  running: boolean;
  lastActivityAt: string | null;
}

/**
 * 侧栏分两段：置顶的按置顶顺序，其余按选的排序（默认 = 导入顺序）。
 * 「等我的优先」：等我件数多的在前，再看有没有在跑，最后看最近活跃。
 */
export function arrangeProjects<T extends Sortable>(rows: readonly T[], pinned: readonly string[], sort: keyof typeof SORT_LABEL): { pinned: T[]; rest: T[] } {
  const byId = new Map(rows.map((r) => [r.id, r]));
  const top = pinned.map((id) => byId.get(id)).filter((r): r is T => !!r);
  const rest = rows.filter((r) => !pinned.includes(r.id));
  const recent = (a: T, b: T) => (b.lastActivityAt ?? "").localeCompare(a.lastActivityAt ?? "");
  if (sort === "recent") rest.sort(recent);
  if (sort === "waiting") rest.sort((a, b) => b.waiting - a.waiting || Number(b.running) - Number(a.running) || recent(a, b));
  return { pinned: top, rest };
}
