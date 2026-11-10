import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { useProjects } from "@/hooks/useProjects";
import { useAllTasks } from "@/hooks/useAllTasks";
import { useUiStore } from "@/stores/uiStore";
import { chimeEnabled, setChimeEnabled } from "@/lib/chime";
import { useCelebrateStore } from "@/stores/celebrateStore";
import { OPEN_WEEKLY } from "@/components/weekly/WeeklyReportDialog";
import { PixelBot } from "@/components/pixel/PixelBot";
import { buildItems, rankItems, type PaletteItem } from "@/components/palette/paletteModel";
import { cn } from "@/lib/utils";

const KIND_LABEL: Record<PaletteItem["kind"], string> = { action: "动作", project: "项目", task: "任务" };
const KIND_TONE: Record<PaletteItem["kind"], string> = {
  action: "bg-action-bg text-action",
  project: "bg-[var(--color-bot-kimi-tint)] text-caution",
  task: "bg-app text-t2",
};

interface Props {
  open: boolean;
  onClose: () => void;
  currentProjectId?: string;
}

/** ⌘K 传送门：搜项目、跨项目搜任务、跑常用动作。上下键选，回车走。 */
export function CommandPalette({ open, onClose, currentProjectId }: Props) {
  const navigate = useNavigate();
  const projects = useProjects();
  const openNewTask = useUiStore((s) => s.openNewTask);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const { tasks, key: taskKey } = useAllTasks(open);

  const items = useMemo(
    () => rankItems(buildItems({ projects: projects.data ?? [], tasks, currentProjectId, chimeOn: chimeEnabled() }), query),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- taskKey stands in for the task arrays
    [projects.data, taskKey, currentProjectId, query, open]
  );

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(0);
    const restoreTo = document.activeElement as HTMLElement | null;
    requestAnimationFrame(() => inputRef.current?.focus());
    return () => restoreTo?.focus?.();
  }, [open]);

  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const choose = (item: PaletteItem | undefined) => {
    if (!item) return;
    onClose();
    if (item.to) navigate(item.to);
    if (item.run === "newTask" && currentProjectId) openNewTask(currentProjectId);
    if (item.run === "toggleChime") setChimeEnabled(!chimeEnabled());
    if (item.run === "toggleSidebar") toggleSidebar();
    if (item.run === "weekly") requestAnimationFrame(() => window.dispatchEvent(new Event(OPEN_WEEKLY)));
    if (item.run === "shortcuts") requestAnimationFrame(() => window.dispatchEvent(new Event("agentflow:shortcuts")));
    if (item.run === "fireworks") useCelebrateStore.getState().celebrate({ text: "辛苦啦，大家！", projectId: currentProjectId });
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(i + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };

  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex justify-center bg-t1/20 px-4 pt-[12vh] backdrop-blur-[2px]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="传送门"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={onKey}
        className="flex h-fit max-h-[70vh] w-full max-w-[560px] flex-col overflow-hidden rounded-overlay bg-raised shadow-float ring-1 ring-line"
      >
        <div className="flex items-center gap-2.5 border-b border-dashed border-line-strong/60 px-4 py-3">
          <PixelBot member="claude_code" mode={query ? "work" : "idle"} size={26} decorative />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="去哪儿？搜项目、任务，或输入动作…"
            aria-label="搜索项目、任务和动作"
            aria-controls="palette-list"
            aria-activedescendant={items[active] ? `palette-${items[active].id}` : undefined}
            className="min-w-0 flex-1 bg-transparent text-body font-semibold text-t1 outline-none placeholder:text-t3"
          />
          <kbd className="rounded-row bg-app px-1.5 py-0.5 font-pixel text-micro text-t3">ESC</kbd>
        </div>
        <ul id="palette-list" ref={listRef} role="listbox" className="min-h-0 flex-1 overflow-y-auto p-1.5">
          {items.length === 0 && <li className="px-3 py-6 text-center text-meta text-t3">没找到，换个词试试？</li>}
          {items.map((item, i) => (
            <li
              key={item.id}
              id={`palette-${item.id}`}
              data-index={i}
              role="option"
              aria-selected={i === active}
              onMouseMove={() => setActive(i)}
              onClick={() => choose(item)}
              className={cn("flex cursor-pointer items-center gap-2.5 rounded-row px-2.5 py-2", i === active ? "bg-action-bg" : "hover:bg-app")}
            >
              <span className={cn("w-9 shrink-0 rounded-row py-0.5 text-center text-micro font-extrabold", KIND_TONE[item.kind])}>{KIND_LABEL[item.kind]}</span>
              <span className={cn("min-w-0 flex-1 truncate text-body", i === active ? "font-extrabold text-t1" : "font-semibold text-t1/90")}>{item.title}</span>
              {item.urgent && <span className="shrink-0 rounded-pill bg-status-human px-1.5 text-micro font-bold text-white">等你</span>}
              <span className="max-w-[45%] shrink-0 truncate text-micro font-semibold text-t3">{item.hint}</span>
            </li>
          ))}
        </ul>
        <div className="flex items-center gap-3 border-t border-line px-4 py-2 text-micro font-semibold text-t3">
          <span>↑↓ 选择</span>
          <span>↵ 前往</span>
          <span className="ml-auto">⌘N 新任务 · ⌘, 设置 · ⌘[ 返回</span>
        </div>
      </div>
    </div>,
    document.body
  );
}
