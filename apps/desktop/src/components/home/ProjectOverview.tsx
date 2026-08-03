import type { ReactNode } from "react";
import { Download, GitBranch, Plus } from "lucide-react";
import type { Project } from "@/generated/bindings";
import { Button } from "@/components/ui/button";
import type { HomeView } from "@/lib/homeTasks";
import { cn } from "@/lib/utils";

export type OverviewCounts = Record<HomeView, number>;

const SUMMARY: Array<{ view: HomeView; label: string }> = [
  { view: "all", label: "全部任务" },
  { view: "attention", label: "需要你" },
  { view: "running", label: "运行中" },
  { view: "done", label: "已完结" },
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
  showStats?: boolean;
}) {
  return (
    <div className="flex flex-col gap-3">
      <section className="border-y border-line/80 bg-panel/45 px-1 py-2.5" aria-label="项目工具栏">
        <div className="flex min-h-14 flex-wrap items-center justify-between gap-x-5 gap-y-2">
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-page font-semibold tracking-tight text-t1">
              {project?.name ?? "项目"}
            </h1>
            <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-meta text-t3">
              {project && (
                <span className="inline-flex items-center gap-1 font-mono">
                  <GitBranch className="size-3.5" aria-hidden /> {project.defaultBranch}
                </span>
              )}
              <span>策略：按任务设置</span>
              {environmentSlot}
              {permissionSlot}
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <Button variant="outline" onClick={onExport}>
              <Download className="size-4" /> 导出审计
            </Button>
            <Button variant="primary" onClick={onNew} title="新建任务 (⌘N)">
              <Plus className="size-4" /> 新建任务
            </Button>
          </div>
        </div>
      </section>

      {showStats && (
        <div
          className="flex min-h-10 overflow-x-auto rounded-control border border-line bg-panel p-1"
          role="tablist"
          aria-label="任务状态筛选"
        >
          {SUMMARY.map((item) => {
            const selected = view === item.view;
            const running = item.view === "running" && counts.running > 0;
            return (
              <button
                key={item.view}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => onViewChange(item.view)}
                className={cn(
                  "flex h-8 min-w-max flex-1 items-center justify-center gap-1.5 rounded-row px-3 text-meta font-medium transition-colors",
                  selected ? "bg-raised text-t1" : "text-t3 hover:bg-raised/60 hover:text-t1"
                )}
              >
                {running && <span className="size-1.5 animate-pulse-dot rounded-circle bg-status-running" aria-hidden />}
                <span>{item.label}</span>
                <span className={cn("min-w-[2ch] tabular-nums", selected ? "text-t1" : "text-t3")}>{counts[item.view]}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
