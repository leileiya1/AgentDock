import type { TaskSummary } from "@/generated/bindings";
import { buildSquad } from "@/lib/squad";
import { cn } from "@/lib/utils";
import { PixelBot, memberLabel, memberShortLabel } from "@/components/pixel/PixelBot";

/**
 * Who is on the team today and what they are doing. Only members with a real
 * running task animate; details live in the hover bubble so the strip stays short.
 */
export function SquadStrip({ tasks, onShowAttention }: { tasks: TaskSummary[]; onShowAttention?: () => void }) {
  const { slots, waitingForYou } = buildSquad(tasks);

  return (
    <section
      aria-label="小队状态"
      className="flex flex-wrap items-center gap-x-2 gap-y-1 rounded-card border border-line bg-raised px-3.5 pb-2 pt-7"
    >
      <span className="-mt-4 mr-1.5 font-pixel text-micro text-t3">SQUAD</span>
      {slots.map((slot) => {
        const name = memberLabel(slot.member);
        const status = slot.working ? `正在处理 ${slot.working}` : "待命";
        return (
          <div
            key={slot.member}
            tabIndex={0}
            aria-label={`${name}：${status}`}
            className="group relative flex min-w-18 flex-col items-center gap-0.5 rounded-control px-1.5 py-1 outline-none hover:bg-app focus-visible:bg-app"
          >
            <span
              className={cn(
                "pointer-events-none absolute bottom-full left-1/2 z-10 -translate-x-1/2 whitespace-nowrap rounded-row px-2 py-0.5 text-meta font-semibold transition-opacity",
                slot.working
                  ? "bg-status-running text-white opacity-100"
                  : "bg-t1 text-raised opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100"
              )}
              aria-hidden
            >
              {slot.working ?? "待命"}
            </span>
            <PixelBot member={slot.member} mode={slot.mode} size={32} decorative />
            <span className={cn("max-w-20 truncate text-micro font-bold", slot.working ? "text-status-running" : "text-t3")}>
              {memberShortLabel(slot.member)}
            </span>
          </div>
        );
      })}
      <button
        type="button"
        onClick={onShowAttention}
        className={cn(
          "-mt-4 ml-auto flex items-center gap-2.5 rounded-section py-1.5 pl-2 pr-3.5 transition-colors",
          waitingForYou > 0 ? "bg-status-human-bg hover:bg-status-human-bg/70" : "bg-status-success-bg"
        )}
        aria-label={waitingForYou > 0 ? `有 ${waitingForYou} 件事等你处理` : "现在没有需要你处理的事"}
      >
        <PixelBot member="you" mode={waitingForYou > 0 ? "hand" : "done"} size={32} decorative />
        <span className="font-bold text-t1">你</span>
        <span
          className={cn(
            "rounded-pill px-2 py-0.5 text-meta font-bold text-white",
            waitingForYou > 0 ? "animate-pop-in bg-status-human" : "bg-status-success"
          )}
        >
          {waitingForYou > 0 ? `${waitingForYou} 件事` : "没事"}
        </span>
      </button>
    </section>
  );
}
