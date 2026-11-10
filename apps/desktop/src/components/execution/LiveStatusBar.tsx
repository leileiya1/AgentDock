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
    // A task may move directly from development to review while both phases have the same
    // `running` tone. Reset the local one-second ticker to the new phase's server-derived
    // elapsed base; otherwise development time is incorrectly carried into review.
    setTick(0);
    if (status?.tone !== "running") return;
    const timer = setInterval(() => setTick((t) => t + 1), 1_000);
    return () => clearInterval(timer);
  }, [status?.tone, status?.headline, status?.elapsedSecs]);

  if (!status) return null;

  const running = status.tone === "running";
  const elapsed = running && status.elapsedSecs != null ? status.elapsedSecs + tick : status.elapsedSecs;

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "flex flex-wrap items-center gap-x-3 gap-y-1 rounded-section px-3.5 py-2 text-body",
        running && "bg-status-running-bg",
        status.tone === "attention" && "bg-status-human-bg",
        status.tone === "failed" && "bg-status-danger-bg",
        (status.tone === "pending" || status.tone === "ok" || status.tone === "info") && "border border-line bg-raised"
      )}
    >
      <StateMark state={status.tone} iconOnly />
      <span className="font-bold text-t1">{status.headline}</span>
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
