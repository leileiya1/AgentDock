import type { ReactNode } from "react";
import type { Project } from "@/generated/bindings";
import { Button } from "@/components/ui/button";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import type { HomeView } from "@/lib/homeTasks";
import { cn } from "@/lib/utils";

export type OverviewCounts = Record<HomeView, number>;

const SUMMARY: Array<{ view: HomeView; label: string; dot: string }> = [
  { view: "attention", label: "轮到你", dot: "bg-status-human" },
  { view: "running", label: "运行中", dot: "bg-status-running" },
  { view: "done", label: "已完结", dot: "bg-status-success" },
  { view: "all", label: "全部任务", dot: "bg-status-idle" },
];

export function ProjectOverview({
  project,
  counts,
  view,
  onViewChange,
  onNew,
  onExport,
  permissionSlot,
  environmentSlot,
  squadSlot,
  showStats = true,
}: {
  project: Project | undefined;
  counts: OverviewCounts;
  view: HomeView;
  onViewChange: (view: HomeView) => void;
  onNew: () => void;
  onExport: () => void;
  permissionSlot?: ReactNode;
  environmentSlot?: ReactNode;
  squadSlot?: ReactNode;
  showStats?: boolean;
}) {
  return (
    <div className="flex flex-col gap-3.5">
      <section aria-label="项目工具栏" className="flex flex-wrap items-center justify-between gap-x-5 gap-y-2">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-page font-extrabold tracking-tight text-t1">{project?.name ?? "项目"}</h1>
          <div className="mt-1.5 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-meta text-t2">
            {project && (
              <span className="inline-flex items-center gap-1.5 rounded-row bg-status-idle-bg px-2 py-0.5 font-mono" title="默认分支">
                <PixelIcon name="branch" size={10} /> {project.defaultBranch}
              </span>
            )}
            <span>策略：按任务设置</span>
            {environmentSlot}
            {permissionSlot}
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <Button variant="outline" onClick={onExport} title="导出审计">
            <PixelIcon name="download" size={11} /> 导出审计
          </Button>
          <Button variant="primary" onClick={onNew} title="新建任务 (⌘N)">
            <PixelIcon name="plus" size={11} /> 新建任务
          </Button>
        </div>
      </section>

      {squadSlot}

      {showStats && (
        <div
          className="flex min-h-10 self-start overflow-x-auto rounded-section bg-status-idle-bg p-1"
          role="tablist"
          aria-label="任务状态筛选"
        >
          {SUMMARY.map((item) => {
            const selected = view === item.view;
            const live = item.view === "running" && counts.running > 0;
            return (
              <button
                key={item.view}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => onViewChange(item.view)}
                className={cn(
                  "flex h-8 min-w-max items-center justify-center gap-1.5 rounded-control px-3 text-meta font-bold transition-colors",
                  selected ? "bg-raised text-t1 shadow-press" : "text-t2 hover:bg-raised/60 hover:text-t1"
                )}
              >
                <span className={cn("size-1.5 rounded-circle", item.dot, live && "animate-px-bob")} aria-hidden />
                <span>{item.label}</span>
                <span className="min-w-[2ch] font-mono tabular-nums">{counts[item.view]}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
