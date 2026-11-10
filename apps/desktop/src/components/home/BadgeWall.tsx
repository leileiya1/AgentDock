import type { Badge } from "@/components/home/paperModel";
import { cn } from "@/lib/utils";

const GLYPH: Record<string, string> = { first: "1", ten: "10", streak: "3x", inbox: "0", crew: "3+", owl: "zz" };

/** 成就墙：像素奖章，拿到的发金光，没拿到的灰着并显示进度环。悬停看怎么拿。 */
export function BadgeWall({ badges }: { badges: Badge[] }) {
  const got = badges.filter((b) => b.earned).length;
  return (
    <div>
      <div className="mb-1.5 text-micro font-extrabold tracking-wider text-t3">成就墙 · {got} / {badges.length}</div>
      <ul className="flex flex-wrap gap-2.5">
        {badges.map((b) => (
          <li key={b.key} className="group relative flex w-14 flex-col items-center gap-1" title={`${b.name}：${b.hint}`}>
            <span
              className="grid size-11 place-items-center rounded-circle p-[3px]"
              style={{
                background: b.earned
                  ? "var(--color-bot-kimi)"
                  : `conic-gradient(var(--color-action) ${Math.round(b.progress * 360)}deg, var(--color-line) 0deg)`,
              }}
            >
              <span
                className={cn(
                  "grid size-full place-items-center rounded-circle font-pixel text-meta transition-transform group-hover:scale-110",
                  b.earned ? "bg-[var(--color-bot-kimi-light)] text-caution shadow-[inset_0_-3px_0_var(--color-bot-kimi)] animate-px-bob" : "bg-app text-t3"
                )}
              >
                {GLYPH[b.key] ?? "★"}
              </span>
            </span>
            <span className={cn("w-full truncate text-center text-micro font-bold", b.earned ? "text-t1" : "text-t3")}>{b.name}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
