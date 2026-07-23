import type { ReactNode } from "react";
import { motion } from "motion/react";
import { Activity, CheckCircle2, Download, GitBranch, LayoutGrid, Plus, Sparkles } from "lucide-react";
import type { Project } from "@/generated/bindings";
import { AnimatedNumber } from "@/components/home/AnimatedNumber";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export interface OverviewCounts {
  total: number;
  attention: number;
  active: number;
  done: number;
}

function greeting(date = new Date()): string {
  const h = date.getHours();
  if (h < 5) return "夜深了";
  if (h < 11) return "早上好";
  if (h < 13) return "中午好";
  if (h < 18) return "下午好";
  return "晚上好";
}

/**
 * 首页概览 (产品化改版). 顶部一条暖色 hero + 一排会滚动计数的 KPI 卡，用 Motion 做
 * 交错入场与悬停浮起，填满原本空旷的首页。全部走 Tailwind + 设计令牌，保持米白底 + 橙强调。
 */
export function ProjectOverview({
  project,
  counts,
  onNew,
  onExport,
  permissionSlot,
  showStats = true,
}: {
  project: Project | undefined;
  counts: OverviewCounts;
  onNew: () => void;
  onExport: () => void;
  permissionSlot?: ReactNode;
  /** 空项目时隐藏 KPI 卡（避免一排 0 显得冷清），改由 HomeEmpty 承担欢迎区。 */
  showStats?: boolean;
}) {
  return (
    <div className="flex flex-col gap-4">
      {/* ── Hero ─────────────────────────────────────────────── */}
      <motion.section
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
        className="relative overflow-hidden rounded-[16px] border border-line/70 bg-gradient-to-br from-raised via-panel to-app px-6 py-5 shadow-[var(--shadow-raised)]"
      >
        {/* 暖橙光晕 + 缓慢扫过的高光，制造「有生命」的质感 */}
        <span className="pointer-events-none absolute -right-16 -top-24 size-64 rounded-full bg-[radial-gradient(closest-side,rgba(194,94,60,0.20),transparent)] blur-2xl" aria-hidden />
        <span className="pointer-events-none absolute -bottom-24 -left-10 size-56 rounded-full bg-[radial-gradient(closest-side,rgba(169,75,43,0.10),transparent)] blur-2xl" aria-hidden />
        <span className="pointer-events-none absolute inset-y-0 -left-1/3 w-1/3 -skew-x-12 bg-gradient-to-r from-transparent via-white/45 to-transparent animate-sheen motion-reduce:hidden" aria-hidden />

        <div className="relative flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-1.5 text-[12px] font-medium text-human">
              <Sparkles className="size-3.5" aria-hidden />
              {greeting()}，欢迎回来
            </div>
            <h1 className="mt-1 truncate text-[26px] font-semibold tracking-tight text-t1">
              {project?.name ?? "项目"}
            </h1>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-t3">
              {project && (
                <span className="inline-flex items-center gap-1 font-mono">
                  <GitBranch className="size-3.5" aria-hidden /> {project.defaultBranch}
                </span>
              )}
              {permissionSlot && <span className="text-line-strong">·</span>}
              {permissionSlot}
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            <Button variant="outline" onClick={onExport}>
              <Download className="size-4" /> 导出审计
            </Button>
            <motion.div whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.97 }}>
              <Button variant="primary" onClick={onNew} title="新建任务 (⌘N)">
                <Plus className="size-4" /> 新建任务
              </Button>
            </motion.div>
          </div>
        </div>
      </motion.section>

      {/* ── KPI 卡 ───────────────────────────────────────────── */}
      {showStats && (
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatTile index={0} label="全部任务" value={counts.total} icon={<LayoutGrid className="size-4" aria-hidden />} />
        <StatTile index={1} label="需要你" value={counts.attention} accent icon={<Sparkles className="size-4" aria-hidden />} />
        <StatTile index={2} label="进行中" value={counts.active} icon={<Activity className="size-4" aria-hidden />} live={counts.active > 0} />
        <StatTile index={3} label="已完结" value={counts.done} icon={<CheckCircle2 className="size-4" aria-hidden />} />
      </div>
      )}
    </div>
  );
}

function StatTile({
  label,
  value,
  icon,
  accent,
  live,
  index,
}: {
  label: string;
  value: number;
  icon: ReactNode;
  accent?: boolean;
  live?: boolean;
  index: number;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.1 + index * 0.06, duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
      whileHover={{ y: -3 }}
      className={cn(
        "group relative overflow-hidden rounded-[12px] border px-4 py-3 shadow-[var(--shadow-raised)] transition-colors",
        accent ? "border-human/40 bg-human-bg/60" : "border-line/70 bg-panel/80 hover:border-line-strong"
      )}
    >
      {/* 顶部一道细高光作为光源 */}
      <span className={cn("pointer-events-none absolute inset-x-0 top-0 h-px", accent ? "bg-gradient-to-r from-transparent via-human/50 to-transparent" : "bg-gradient-to-r from-transparent via-white/70 to-transparent")} aria-hidden />
      <div className="flex items-center justify-between">
        <span className={cn("text-[12px] font-medium", accent ? "text-human" : "text-t3")}>{label}</span>
        <span className={cn("transition-transform group-hover:scale-110", accent ? "text-human" : "text-t3/70")}>{icon}</span>
      </div>
      <div className="mt-1.5 flex items-baseline gap-1.5">
        <AnimatedNumber
          value={value}
          className={cn("text-[28px] font-semibold leading-none tabular-nums", accent ? "text-human" : "text-t1")}
        />
        {live && (
          <span className="mb-0.5 inline-flex items-center gap-1 text-[11px] font-medium text-run">
            <span className="size-1.5 animate-pulse-dot rounded-full bg-run" /> 活跃
          </span>
        )}
      </div>
    </motion.div>
  );
}
