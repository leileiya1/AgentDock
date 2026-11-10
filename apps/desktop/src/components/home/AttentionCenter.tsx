import { forwardRef, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { TaskSummary } from "@/generated/bindings";
import { buildAttentionItems, groupAttentionItems, type AttentionItem, type AttentionSeverity } from "@/lib/attention";
import { attentionNav } from "@/lib/attentionNav";
import { relativeTime, taskCode } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateBadge } from "@/components/StateBadge";
import { PixelBot } from "@/components/pixel/PixelBot";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { taskAvatar } from "@/lib/squad";

const DEFAULT_LIMIT = 8;

const SEVERITY: Record<AttentionSeverity, { label: string; className: string } | null> = {
  high: { label: "高影响", className: "text-status-danger" },
  medium: { label: "需尽快", className: "text-status-human" },
  normal: null,
};

export function AttentionCenter({ tasks, onOpen, density = "comfortable" }: { tasks: TaskSummary[]; onOpen: (task: TaskSummary) => void; density?: "comfortable" | "compact" }) {
  const items = useMemo(() => buildAttentionItems(tasks), [tasks]);
  const [showAll, setShowAll] = useState(false);
  const [focusedId, setFocusedId] = useState<string | null>(null);
  const rowRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const visible = showAll ? items : items.slice(0, DEFAULT_LIMIT);
  const activeId = visible.some((item) => item.task.id === focusedId) ? focusedId : visible[0]?.task.id ?? null;
  const grouped = showAll || items.length <= DEFAULT_LIMIT;
  const groups = grouped
    ? groupAttentionItems(visible)
    : [{ key: "visible", label: "", items: visible }];

  const onRowKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const result = attentionNav(event.key, index, visible.length);
    if (!result) return;
    event.preventDefault();
    const item = visible[result.index];
    if (!item) return;
    if (result.type === "open") {
      onOpen(item.task);
      return;
    }
    setFocusedId(item.task.id);
    rowRefs.current[result.index]?.focus();
  };

  return (
    <section className="overflow-hidden rounded-card border border-line bg-raised" aria-labelledby="attention-title">
      <div className="flex min-h-11 items-center justify-between gap-3 border-b border-line/70 px-4 py-2">
        <div className="flex min-w-0 items-baseline gap-2">
          <h2 id="attention-title" className="text-section font-semibold text-t1">需要你处理</h2>
          <span className="tabular-nums text-meta text-t3">{items.length}</span>
        </div>
        {items.length > 0 && <span className="text-meta text-t3">按影响和更新时间排序</span>}
      </div>

      {items.length === 0 ? (
        <div className="flex items-center gap-3 px-4 py-5 text-body text-t2">
          <PixelBot member="you" mode="done" size={28} decorative />
          当前没有等待你处理的事项。
        </div>
      ) : (
        <div>
          {groups.map((group) => (
            <div key={group.key}>
              {grouped && group.items.length > 1 && (
                <div className="border-b border-line/60 bg-app px-4 py-1.5 text-meta font-bold text-t3">
                  {group.label} · <span className="tabular-nums">{group.items.length}</span>
                </div>
              )}
              {group.items.map((item) => {
                const index = visible.findIndex((candidate) => candidate.task.id === item.task.id);
                return (
                  <AttentionRow
                    key={item.task.id}
                    item={item}
                    tabIndex={item.task.id === activeId ? 0 : -1}
                    ref={(node) => { rowRefs.current[index] = node; }}
                    onFocus={() => setFocusedId(item.task.id)}
                    onKeyDown={(event) => onRowKeyDown(event, index)}
                    onOpen={() => onOpen(item.task)}
                    density={density}
                  />
                );
              })}
            </div>
          ))}
        </div>
      )}

      {items.length > DEFAULT_LIMIT && (
        <div className="border-t border-line/70 px-4 py-2 text-right">
          <button
            type="button"
            onClick={() => setShowAll((current) => !current)}
            className="text-meta font-medium text-status-human hover:underline"
          >
            {showAll ? `收起到 ${DEFAULT_LIMIT} 项` : `查看全部 ${items.length} 项`}
          </button>
        </div>
      )}
    </section>
  );
}

const AttentionRow = forwardRef<
  HTMLButtonElement,
  {
    item: AttentionItem;
    tabIndex: number;
    onFocus: () => void;
    onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
    onOpen: () => void;
    density: "comfortable" | "compact";
  }
>(({ item, tabIndex, onFocus, onKeyDown, onOpen, density }, ref) => {
  const severity = SEVERITY[item.severity];
  const avatar = taskAvatar(item.task);
  return (
    <button
      ref={ref}
      type="button"
      tabIndex={tabIndex}
      data-task-row
      onFocus={onFocus}
      onKeyDown={onKeyDown}
      onClick={onOpen}
      className={cn(
        "group grid w-full animate-row-in grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3.5 border-b border-line/60 px-4 text-left transition-colors last:border-b-0 hover:bg-status-human-bg/40 focus-visible:bg-status-human-bg/40",
        density === "compact" ? "py-1.5" : "py-2.5"
      )}
    >
      <PixelBot member={avatar.member} mode={avatar.mode} size={32} />
      <span className="min-w-0">
        <span className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate text-body font-bold text-t1">{item.task.title}</span>
          {severity && (
            <span className={cn("inline-flex shrink-0 items-center", severity.className)} title={severity.label}>
              <PixelIcon name="bolt" size={11} label={severity.label} />
            </span>
          )}
          <StateBadge status={item.task.status} size="sm" />
        </span>
        <span className="mt-0.5 flex min-w-0 items-center gap-2 text-meta">
          <span className="shrink-0 font-mono text-t3">{taskCode(item.task.seq)}</span>
          <span className="min-w-0 truncate text-t2">{item.reason}</span>
          <span className="inline-flex shrink-0 items-center gap-1 whitespace-nowrap tabular-nums text-t3">
            <PixelIcon name="hourglass" size={9} />更新于 {relativeTime(item.task.updatedAt)}
          </span>
        </span>
      </span>
      <span className="inline-flex shrink-0 items-center gap-1 rounded-control border border-line-strong bg-raised px-3 py-1.5 text-meta font-bold text-t1 shadow-press transition-colors group-hover:border-status-human/40 group-hover:text-status-human">
        {item.action}
        <PixelIcon name="chevronRight" size={9} />
      </span>
    </button>
  );
});
AttentionRow.displayName = "AttentionRow";
