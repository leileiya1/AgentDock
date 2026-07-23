import { useEffect, useState } from "react";
import type { LiveStatus } from "@/lib/execution/liveStatus";
import { formatElapsed } from "@/lib/execution/liveStatus";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { StateMark } from "./StateMark";

/**
 * 当前 revision 顶部的持续状态条 (05 §3.4)。运行中每秒推进已用时；
 * 超过 10 秒没有新活动时明确说明「仍在运行」，而不是看起来卡死。
 */
export function LiveStatusBar({ status }: { status: LiveStatus | null }) {
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (status?.tone !== "running") return;
    const timer = setInterval(() => setTick((t) => t + 1), 1_000);
    return () => clearInterval(timer);
  }, [status?.tone]);

  if (!status) return null;

  const running = status.tone === "running";
  const elapsed = running && status.elapsedSecs != null ? status.elapsedSecs + tick : status.elapsedSecs;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "flex flex-wrap items-center gap-x-3 gap-y-1 rounded-[var(--radius-panel)] border px-3 py-2 text-[13px]",
        running && "border-run/40 bg-run/5",
        status.tone === "attention" && "border-human/50 bg-human-bg",
        status.tone === "pending" && "border-line bg-panel/60"
      )}
    >
      <StateMark state={status.tone} iconOnly />
      <span className="font-medium text-t1">{status.headline}</span>
      {elapsed != null && (
        <span className="tabular-nums text-t2">已运行 {formatElapsed(elapsed)}</span>
      )}
      {status.detail && <span className="text-t2">{status.detail}</span>}
      {status.stalled && status.lastActivityAt && (
        <span className="text-t3">仍在运行，最近活动于 {relativeTime(status.lastActivityAt)}</span>
      )}
    </div>
  );
}
