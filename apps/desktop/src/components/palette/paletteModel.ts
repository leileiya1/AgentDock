import type { Project, TaskSummary } from "@/generated/bindings";
import { groupForStatus } from "@/copy/status";

export type PaletteKind = "action" | "project" | "task";

export interface PaletteItem {
  id: string;
  kind: PaletteKind;
  title: string;
  /** 右侧的小字：项目名、状态、快捷键。 */
  hint: string;
  /** 额外参与匹配的词（拼音缩写、英文别名）。 */
  keywords?: string;
  /** 等你处理的任务排在前面。 */
  urgent?: boolean;
  to?: string;
  run?: "newTask" | "toggleChime" | "toggleSidebar" | "fireworks" | "shortcuts" | "weekly";
}

/**
 * 模糊匹配：query 的每个字符按顺序出现在 text 里即命中。
 * 连续命中、命中在词首、命中在开头都加分；没命中返回 -1。
 */
export function fuzzyScore(query: string, text: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const t = text.toLowerCase();
  const direct = t.indexOf(q);
  if (direct >= 0) return 100 - Math.min(direct, 40) + (direct === 0 ? 20 : 0);
  let score = 0;
  let from = 0;
  let prev = -2;
  for (const ch of q) {
    if (ch === " ") continue;
    const at = t.indexOf(ch, from);
    if (at < 0) return -1;
    score += at === prev + 1 ? 6 : 1;
    if (at === 0 || /[\s\-_/·]/.test(t[at - 1] ?? "")) score += 3;
    prev = at;
    from = at + 1;
  }
  return score;
}

export function rankItems(items: PaletteItem[], query: string, limit = 40): PaletteItem[] {
  if (!query.trim()) {
    // 空查询：先动作、再等你的任务、再项目、最后最近的任务。
    const order = (i: PaletteItem) => (i.kind === "action" ? 0 : i.urgent ? 1 : i.kind === "project" ? 2 : 3);
    return [...items].sort((a, b) => order(a) - order(b)).slice(0, limit);
  }
  return items
    .map((item) => ({ item, s: fuzzyScore(query, `${item.title} ${item.hint} ${item.keywords ?? ""}`) }))
    .filter((x) => x.s >= 0)
    .map((x) => ({ ...x, s: x.s + (x.item.urgent ? 4 : 0) }))
    .sort((a, b) => b.s - a.s)
    .slice(0, limit)
    .map((x) => x.item);
}

export const taskCode = (seq: number) => `TASK-${String(seq).padStart(3, "0")}`;

interface Sources {
  projects: Project[];
  tasks: TaskSummary[];
  currentProjectId?: string;
  chimeOn: boolean;
}

/** 把项目、任务和常用动作摊平成一张可搜索的清单。 */
export function buildItems({ projects, tasks, currentProjectId, chimeOn }: Sources): PaletteItem[] {
  const nameOf = new Map(projects.map((p) => [p.id, p.name]));
  const actions: PaletteItem[] = [
    { id: "a:town", kind: "action", title: "去项目小镇", hint: "全部项目一张地图", keywords: "town map xz", to: "/town" },
    { id: "a:home", kind: "action", title: "回首页", hint: "", keywords: "home sy", to: "/" },
    { id: "a:settings", kind: "action", title: "打开设置", hint: "⌘ ,", keywords: "settings sz", to: "/settings" },
    { id: "a:import", kind: "action", title: "导入项目", hint: "盖一栋新房子", keywords: "import onboarding dr", to: "/onboarding" },
    { id: "a:chime", kind: "action", title: chimeOn ? "关闭提示音" : "打开提示音", hint: "任务完成 / 等你时叮一声", keywords: "sound chime ts", run: "toggleChime" },
    { id: "a:weekly", kind: "action", title: "本周成绩单", hint: "合并、返工率、MVP", keywords: "weekly report zb cjd", run: "weekly" },
    { id: "a:keys", kind: "action", title: "键盘快捷键", hint: "?", keywords: "shortcuts keyboard kjj", run: "shortcuts" },
    { id: "a:fireworks", kind: "action", title: "放个烟花", hint: "犒劳一下大家", keywords: "fireworks party yh", run: "fireworks" },
    { id: "a:sidebar", kind: "action", title: "折叠 / 展开侧栏", hint: "", keywords: "sidebar cl", run: "toggleSidebar" },
  ];
  if (currentProjectId) {
    actions.unshift({ id: "a:new", kind: "action", title: `在「${nameOf.get(currentProjectId) ?? "当前项目"}」新建任务`, hint: "⌘ N", keywords: "new task xj", run: "newTask" });
  }
  const projectItems: PaletteItem[] = projects.map((p) => ({ id: `p:${p.id}`, kind: "project", title: p.name, hint: "项目", keywords: "project", to: `/p/${p.id}` }));
  const taskItems: PaletteItem[] = [...tasks]
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
    .map((t) => {
      const group = groupForStatus(t.status);
      return {
        id: `t:${t.id}`,
        kind: "task" as const,
        title: t.title,
        hint: `${nameOf.get(t.projectId) ?? ""} · ${taskCode(t.seq)}`,
        keywords: group === "attention" ? "等你 waiting" : group === "done" ? "完成 done" : "进行中 running",
        urgent: group === "attention",
        to: `/p/${t.projectId}/t/${t.id}`,
      };
    });
  return [...actions, ...projectItems, ...taskItems];
}
