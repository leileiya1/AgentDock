import { useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, ChevronRight, GitMerge, KeyRound, ShieldAlert, UserCheck } from "lucide-react";
import type { TaskSummary } from "@/generated/bindings";
import { ATTENTION_META, buildAttentionItems, type AttentionKind } from "@/lib/attention";
import { relativeTime, taskCode } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";

type Filter = "all" | AttentionKind;

const ICON = {
  permission: ShieldAlert,
  approval: UserCheck,
  recovery: AlertTriangle,
  conflict: GitMerge,
  delivery: KeyRound,
} satisfies Record<AttentionKind, typeof AlertTriangle>;

export function AttentionCenter({ tasks, onOpen }: { tasks: TaskSummary[]; onOpen: (task: TaskSummary) => void }) {
  const items = useMemo(() => buildAttentionItems(tasks), [tasks]);
  const [filter, setFilter] = useState<Filter>("all");
  const counts = useMemo(() => {
    const result = new Map<AttentionKind, number>();
    for (const item of items) result.set(item.kind, (result.get(item.kind) ?? 0) + 1);
    return result;
  }, [items]);
  const visible = filter === "all" ? items : items.filter((item) => item.kind === filter);

  return (
    <section className="rounded-[var(--radius-panel)] border border-human/25 bg-human-bg/20 p-3.5" aria-labelledby="attention-title">
      <div className="flex flex-wrap items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="size-2 rounded-full bg-human shadow-[0_0_8px_-1px_var(--color-human)]" aria-hidden />
            <h2 id="attention-title" className="text-[14px] font-semibold text-t1">需要你处理</h2>
            <span className="rounded-full bg-human-bg px-2 py-0.5 text-[11px] font-semibold text-human">{items.length}</span>
          </div>
          <p className="mt-1 text-[12px] text-t3">只列出必须由你决定、授权或恢复的事项，并说明下一步。</p>
        </div>
      </div>

      {items.length === 0 ? (
        <div className="mt-3 flex items-center gap-2 rounded-lg border border-dashed border-ok/30 bg-ok/5 px-3 py-3 text-[13px] text-t2">
          <CheckCircle2 className="size-4 text-ok" aria-hidden /> 当前没有等待你处理的事项。
        </div>
      ) : (
        <>
          <div className="mt-3 flex gap-1 overflow-x-auto pb-0.5" role="tablist" aria-label="处理事项筛选">
            <FilterButton label="全部" count={items.length} active={filter === "all"} onClick={() => setFilter("all")} />
            {(Object.keys(ATTENTION_META) as AttentionKind[]).map((kind) => {
              const count = counts.get(kind) ?? 0;
              if (count === 0) return null;
              return (
                <FilterButton
                  key={kind}
                  label={ATTENTION_META[kind].label}
                  count={count}
                  active={filter === kind}
                  onClick={() => setFilter(kind)}
                />
              );
            })}
          </div>

          <div className="mt-2.5 flex flex-col gap-2">
            {visible.map((item) => {
              const Icon = ICON[item.kind];
              return (
                <button
                  key={item.task.id}
                  type="button"
                  onClick={() => onOpen(item.task)}
                  className="group grid w-full grid-cols-[auto_minmax(0,1fr)_auto] gap-x-3 gap-y-1 rounded-[10px] border border-line/75 bg-panel/85 px-3 py-2.5 text-left transition-colors hover:border-human/45 hover:bg-raised"
                >
                  <span className="row-span-2 mt-0.5 grid size-8 place-items-center rounded-lg bg-human-bg text-human">
                    <Icon className="size-4" aria-hidden />
                  </span>
                  <span className="flex min-w-0 flex-wrap items-center gap-2">
                    <span className="font-mono text-[11px] text-t3">{taskCode(item.task.seq)}</span>
                    <span className="min-w-0 truncate text-[13px] font-semibold text-t1">{item.task.title}</span>
                    <StateBadge status={item.task.status} size="sm" />
                  </span>
                  <span className="row-span-2 flex items-center gap-2 self-center pl-2">
                    <span className="hidden text-[12px] font-medium text-human sm:inline">{ATTENTION_META[item.kind].action}</span>
                    <ChevronRight className="size-4 text-t3 transition-transform group-hover:translate-x-0.5 group-hover:text-human" aria-hidden />
                  </span>
                  <span className="min-w-0 text-[12px] leading-relaxed text-t2">
                    {item.reason} <span className="text-t3">下一步：{item.nextStep}</span>
                    <span className="ml-2 whitespace-nowrap text-[11px] text-t3">{relativeTime(item.task.updatedAt)}</span>
                  </span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </section>
  );
}

function FilterButton({ label, count, active, onClick }: { label: string; count: number; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={cn(
        "shrink-0 rounded-full border px-2.5 py-1 text-[11px] transition-colors",
        active ? "border-human/50 bg-human-bg text-human" : "border-line bg-panel text-t3 hover:text-t1"
      )}
    >
      {label} {count}
    </button>
  );
}
