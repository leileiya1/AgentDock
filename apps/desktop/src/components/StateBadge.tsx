import type { TaskStatus } from "@/generated/bindings";
import { STATUS_COPY, type StatusTone } from "@/copy/status";
import { cn } from "@/lib/utils";

const TONE: Record<StatusTone, { text: string; ring: string; dot: string; wash?: string }> = {
  idle: { text: "text-status-idle", ring: "border-line", dot: "bg-status-idle" },
  run: { text: "text-status-running", ring: "border-status-running/45", dot: "bg-status-running", wash: "bg-status-running-bg/70" },
  review: { text: "text-status-review", ring: "border-status-review/45", dot: "bg-status-review" },
  ok: { text: "text-status-success", ring: "border-status-success/45", dot: "bg-status-success" },
  bad: { text: "text-status-danger", ring: "border-status-danger/45", dot: "bg-status-danger" },
  human: { text: "text-status-human", ring: "border-status-human/55", dot: "bg-status-human", wash: "bg-status-human-bg/80" },
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
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-pill border font-medium leading-none",
        size === "sm" ? "px-2 py-1 text-meta" : "px-2.5 py-1 text-meta",
        tone.text,
        tone.ring,
        tone.wash ?? "bg-transparent"
      )}
    >
      <span className={cn("size-1.5 shrink-0 rounded-circle", tone.dot, copy.pulse && "animate-pulse-dot")} />
      {copy.label}
    </span>
  );
}
