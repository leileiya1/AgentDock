import type { Project, TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";

/** 离开多久才算"不在"。 */
export const AWAY_MS = 10 * 60_000;
const KEY = "agentflow.lastSeen";

export function readLastSeen(): number | null {
  try {
    const v = Number(localStorage.getItem(KEY));
    return Number.isFinite(v) && v > 0 ? v : null;
  } catch {
    return null;
  }
}

export function writeLastSeen(at: number): void {
  try {
    localStorage.setItem(KEY, String(at));
  } catch {
    /* per-viewer convenience only */
  }
}

export type RecapBucket = "merged" | "waiting" | "trouble" | "moving";

export interface RecapItem {
  taskId: string;
  projectId: string;
  projectName: string;
  title: string;
  seq: number;
  bucket: RecapBucket;
}

export interface Recap {
  awayMs: number;
  counts: Record<RecapBucket, number>;
  /** 按 合并 → 出问题 → 等你 → 在跑 排，最多 6 条。 */
  items: RecapItem[];
  headline: string;
}

const TROUBLE = new Set<TaskSummary["status"]>(["MERGE_CONFLICT", "ROLLED_BACK"]);
const ORDER: RecapBucket[] = ["merged", "trouble", "waiting", "moving"];

export function bucketOf(status: TaskSummary["status"]): RecapBucket | null {
  if (status === "MERGED") return "merged";
  if (TROUBLE.has(status)) return "trouble";
  const g = groupForStatus(status);
  if (g === "attention") return "waiting";
  if (g === "active") return "moving";
  return null; // 取消等不值得汇报
}

export function awayLabel(ms: number): string {
  const min = Math.round(ms / 60_000);
  if (min < 60) return `${min} 分钟`;
  const h = Math.floor(min / 60);
  if (h < 24) return min % 60 ? `${h} 小时 ${min % 60} 分` : `${h} 小时`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d} 天 ${h % 24} 小时` : `${d} 天`;
}

/** 离开期间有更新的任务按结果分组；离开不够久或什么都没发生时返回 null。 */
export function buildRecap(tasks: TaskSummary[], projects: Project[], lastSeen: number | null, now: number): Recap | null {
  if (lastSeen === null || now - lastSeen < AWAY_MS) return null;
  const nameOf = new Map(projects.map((p) => [p.id, p.name]));
  const items: RecapItem[] = [];
  for (const t of tasks) {
    if (Date.parse(t.updatedAt) <= lastSeen) continue;
    const bucket = bucketOf(t.status);
    if (!bucket) continue;
    items.push({ taskId: t.id, projectId: t.projectId, projectName: nameOf.get(t.projectId) ?? "", title: t.title, seq: t.seq, bucket });
  }
  if (items.length === 0) return null;
  const counts = { merged: 0, waiting: 0, trouble: 0, moving: 0 };
  for (const i of items) counts[i.bucket] += 1;
  items.sort((a, b) => ORDER.indexOf(a.bucket) - ORDER.indexOf(b.bucket));
  const headline =
    counts.trouble > 0 ? `号外！${counts.trouble} 件出了岔子，快来看看`
    : counts.merged > 0 && counts.waiting > 0 ? `好消息 ${counts.merged} 件，还有 ${counts.waiting} 件等你拍板`
    : counts.merged > 0 ? `号外！${counts.merged} 件已经合并上线`
    : counts.waiting > 0 ? `${counts.waiting} 件事在门口等你`
    : `大家没闲着，${counts.moving} 件还在推进`;
  return { awayMs: now - lastSeen, counts, items: items.slice(0, 6), headline };
}
