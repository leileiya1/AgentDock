import { useState } from "react";
import { ChevronRight, ExternalLink } from "lucide-react";
import type { NormalizedEvent } from "@/lib/execution/normalize";
import { absoluteTime, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateMark } from "./StateMark";

/**
 * 系统详情 (05 §3.3). scheduler slot、结果文件、内部同步等永远收在这里；
 * 技术能力不删除，只是不默认打扰普通用户 (05 §1.2)。
 */
export function SystemDetails({ events, onOpenRun }: { events: NormalizedEvent[]; onOpenRun?: (event: NormalizedEvent) => void }) {
  const [open, setOpen] = useState(false);
  if (events.length === 0) return null;

  return (
    <div className="mt-2 border-t border-line/70 pt-2">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[12px] text-t3 transition-colors hover:bg-raised hover:text-t2"
      >
        <ChevronRight className={cn("size-3.5 shrink-0 transition-transform", open && "rotate-90")} aria-hidden />
        系统详情
        <span className="ml-auto tabular-nums">{events.length} 条</span>
      </button>
      {open && (
        <ul className="mt-1 list-none space-y-0.5 pl-6 pr-2">
          {events.map((event) => (
            <li key={event.id} className="flex items-start gap-1.5 py-0.5 text-[12px] text-t3">
              <StateMark state={event.copy.state} iconOnly className="mt-px scale-90" />
              <span className="min-w-0 flex-1">
                {event.copy.label}
                {event.copy.detail && <span className="block text-t3/80">{event.copy.detail}</span>}
              </span>
              <span className="shrink-0 tabular-nums" title={absoluteTime(event.ts)}>
                {relativeTime(event.ts)}
              </span>
              {event.runId && onOpenRun && (
                <button
                  type="button"
                  className="shrink-0 rounded p-0.5 text-run hover:bg-raised"
                  aria-label="查看对应执行日志"
                  title="查看对应执行日志"
                  onClick={() => onOpenRun(event)}
                >
                  <ExternalLink className="size-3" />
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
