import { useEffect } from "react";
import { shouldIgnoreKey, stepIndex } from "@/lib/keys";

/**
 * 任务列表的键盘流：J/K 在所有带 data-task-row 的行之间移焦点（跨分组），
 * 回车就是按钮本身的点击；/ 聚焦搜索框。
 */
export function useListKeys(enabled = true): void {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (shouldIgnoreKey(e)) return;
      if (e.key === "/") {
        const search = document.getElementById("task-search");
        if (search) {
          e.preventDefault();
          search.focus();
        }
        return;
      }
      const rows = [...document.querySelectorAll<HTMLElement>("[data-task-row]")];
      const next = stepIndex(e.key, rows.indexOf(document.activeElement as HTMLElement), rows.length);
      if (next === null) return;
      e.preventDefault();
      rows[next].focus();
      rows[next].scrollIntoView({ block: "nearest" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled]);
}
