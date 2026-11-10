import { cn } from "@/lib/utils";

export type ThemeChoice = "auto" | "day" | "night";

/** 「自动 / 白天 / 夜间」：自动是早 7 点到晚 7 点白天。 */
export function themeFor(choice: ThemeChoice, now = new Date()): "day" | "night" {
  if (choice !== "auto") return choice;
  const h = now.getHours();
  return h >= 7 && h < 19 ? "day" : "night";
}

export function ThemeSwitch({ value, onChange, className }: { value: ThemeChoice; onChange: (v: ThemeChoice) => void; className?: string }) {
  return (
    <div className={cn("flex rounded-control border border-line-strong bg-raised/90 p-0.5 text-micro font-bold", className)} role="radiogroup" aria-label="光线">
      {(["auto", "day", "night"] as const).map((c) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-checked={value === c}
          onClick={() => onChange(c)}
          className={cn("rounded-row px-2.5 py-1", value === c ? "bg-action text-white" : "text-t2 hover:text-t1")}
        >
          {c === "auto" ? "自动" : c === "day" ? "白天" : "夜间"}
        </button>
      ))}
    </div>
  );
}

export function ZoomControls({ pct, onIn, onOut, onReset, className }: { pct: number; onIn: () => void; onOut: () => void; onReset: () => void; className?: string }) {
  const btn = "grid size-7 place-items-center rounded-row hover:bg-app hover:text-t1";
  return (
    <div className={cn("flex items-center gap-0.5 rounded-control border border-line-strong bg-raised/90 p-0.5 text-micro font-bold text-t2", className)}>
      <button type="button" onClick={onOut} className={btn} aria-label="缩小">−</button>
      <button type="button" onClick={onReset} className="min-w-12 rounded-row px-1.5 py-1 font-mono tabular-nums hover:bg-app hover:text-t1" title="复位（0）">
        {pct}%
      </button>
      <button type="button" onClick={onIn} className={btn} aria-label="放大">+</button>
    </div>
  );
}
