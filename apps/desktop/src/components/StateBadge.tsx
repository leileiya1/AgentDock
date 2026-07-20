import type { TaskStatus } from "@/generated/bindings";
import { STATUS_COPY, type StatusTone } from "@/copy/status";
import { cn } from "@/lib/utils";

const TONE: Record<StatusTone, { text: string; ring: string; dot: string; glow?: string }> = {
  idle: { text: "text-idle", ring: "border-line", dot: "bg-idle" },
  run: { text: "text-run", ring: "border-run/40", dot: "bg-run" },
  review: { text: "text-review", ring: "border-review/40", dot: "bg-review" },
  ok: { text: "text-ok", ring: "border-ok/40", dot: "bg-ok" },
  bad: { text: "text-bad", ring: "border-bad/40", dot: "bg-bad" },
  human: {
    text: "text-human",
    ring: "border-human/60",
    dot: "bg-human",
    glow: "bg-human-bg shadow-[0_0_16px_-4px_rgba(232,163,61,0.5)]",
  },
};

interface Props {
  status: TaskStatus;
  size?: "sm" | "md";
}

/** status → {label, color, pulse}. "需要你" family is always amber (02 §6). */
export function StateBadge({ status, size = "md" }: Props) {
  const copy = STATUS_COPY[status];
  const tone = TONE[copy.tone];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border font-medium leading-none",
        size === "sm" ? "px-2 py-1 text-[11px]" : "px-2.5 py-1 text-[12px]",
        tone.text,
        tone.ring,
        tone.glow ?? "bg-transparent"
      )}
    >
      <span
        className={cn(
          "size-1.5 shrink-0 rounded-full",
          tone.dot,
          copy.pulse && "animate-pulse-dot"
        )}
      />
      {copy.label}
    </span>
  );
}
