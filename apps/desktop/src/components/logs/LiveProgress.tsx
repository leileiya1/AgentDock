import { Activity } from "lucide-react";
import type { ReadableEvent } from "@/lib/logs/readable";
import { formatElapsed } from "@/lib/execution/liveStatus";
import { relativeTime } from "@/lib/format";

/**
 * 「实时进展」(05 §4.1). 只在运行中出现，全部是人类可读短句。
 * 超过 10 秒没有新输出时明确说明仍在运行 (05 §4.4)。
 */
export function LiveProgress({
  events,
  elapsedSecs,
  lastActivityAt,
  stalled,
}: {
  events: ReadableEvent[];
  elapsedSecs: number | null;
  lastActivityAt: string | null;
  stalled: boolean;
}) {
  return (
    <section className="rounded-section border border-status-running/30 bg-status-running/5 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Activity className="size-4 shrink-0 text-status-running" aria-hidden />
        <h2 className="text-body font-semibold text-t2">实时进展</h2>
        {elapsedSecs != null && (
          <span className="text-meta tabular-nums text-t2">已运行 {formatElapsed(elapsedSecs)}</span>
        )}
        {stalled && lastActivityAt && (
          <span className="text-meta text-t3">仍在运行，最近活动于 {relativeTime(lastActivityAt)}</span>
        )}
      </div>

      {events.length === 0 ? (
        <p className="mt-2 text-body text-t3">Agent 已启动，还没有输出可读进展。</p>
      ) : (
        <ol className="mt-2 list-none space-y-1.5">
          {events.map((event, index) => (
            <li
              key={`${event.original.ts}-${index}`}
              className="flex gap-2 text-body leading-relaxed text-t1"
            >
              <span className="shrink-0 text-t3" aria-hidden>›</span>
              <span className="min-w-0 whitespace-pre-wrap">{event.text}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
