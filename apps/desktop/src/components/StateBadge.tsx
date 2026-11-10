import type { TaskStatus } from "@/generated/bindings";
import { STATUS_COPY, type StatusTone } from "@/copy/status";
import { cn } from "@/lib/utils";

const TONE: Record<StatusTone, { text: string; dot: string; wash: string }> = {
  idle: { text: "text-status-idle", dot: "bg-status-idle", wash: "bg-status-idle-bg" },
  run: { text: "text-status-running", dot: "bg-status-running", wash: "bg-status-running-bg" },
  review: { text: "text-status-review", dot: "bg-status-review", wash: "bg-status-review-bg" },
  ok: { text: "text-status-success", dot: "bg-status-success", wash: "bg-status-success-bg" },
  bad: { text: "text-status-danger", dot: "bg-status-danger", wash: "bg-status-danger-bg" },
  human: { text: "text-status-human", dot: "bg-status-human", wash: "bg-status-human-bg" },
};

interface Props {
  status: TaskStatus;
  size?: "sm" | "md";
}

export function StateBadge({ status, size = "md" }: Props) {
  const copy = STATUS_COPY[status];
  const tone = TONE[copy.tone];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-pill font-semibold leading-none",
        size === "sm" ? "px-2 py-1 text-meta" : "px-2.5 py-1.5 text-meta",
        tone.text,
        tone.wash
      )}
    >
      <span className={cn("size-1.5 shrink-0 rounded-circle", tone.dot, copy.pulse && "animate-pulse-dot")} />
      {copy.label}
    </span>
  );
}
