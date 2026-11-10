/** 全局单键快捷键（J/K/A/R/?）的公共判断：正在打字、按着修饰键、或有弹窗开着时一律不抢。 */
export function shouldIgnoreKey(e: KeyboardEvent): boolean {
  if (e.metaKey || e.ctrlKey || e.altKey || e.defaultPrevented || e.isComposing) return true;
  const t = e.target as HTMLElement | null;
  if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName) || t.closest?.(".monaco-editor"))) return true;
  return typeof document !== "undefined" && document.querySelector('[aria-modal="true"]') !== null;
}

/** J/K 在列表里走一步；没选中时 J 落到第一行、K 落到最后一行。 */
export function stepIndex(key: string, current: number, count: number): number | null {
  if (count <= 0) return null;
  const k = key.toLowerCase();
  if (k !== "j" && k !== "k") return null;
  if (current < 0) return k === "j" ? 0 : count - 1;
  return k === "j" ? Math.min(current + 1, count - 1) : Math.max(current - 1, 0);
}

export interface Shortcut {
  keys: string[];
  label: string;
}

/** 快捷键速查表（? 打开）。 */
export const SHORTCUTS: Array<{ group: string; items: Shortcut[] }> = [
  {
    group: "到处都能用",
    items: [
      { keys: ["⌘", "K"], label: "传送门：搜项目、任务、动作" },
      { keys: ["⌘", "N"], label: "在当前项目新建任务" },
      { keys: ["⌘", ","], label: "设置" },
      { keys: ["⌘", "["], label: "返回上一页" },
      { keys: ["?"], label: "打开这张速查表" },
    ],
  },
  {
    group: "任务列表",
    items: [
      { keys: ["J"], label: "下一条任务" },
      { keys: ["K"], label: "上一条任务" },
      { keys: ["↵"], label: "打开选中的任务" },
      { keys: ["/"], label: "搜索任务" },
    ],
  },
  {
    group: "等你批准的任务",
    items: [
      { keys: ["A"], label: "批准（会先弹确认框）" },
      { keys: ["R"], label: "驳回并写理由" },
      { keys: ["⌘", "↵"], label: "在确认框里确认批准" },
    ],
  },
];
