import type { ReactNode } from "react";
import { SWATCH_TOKENS, resolveToken } from "@/lib/studio/studioOptions";
import { cn } from "@/lib/utils";

/** 一组标签，带标题和可选说明。 */
export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-meta font-bold text-t1">{label}</span>
        {hint && <span className="text-micro text-t3">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

/** 小药丸按钮：单选（radio）或多选（checkbox）都用它。 */
export function Pill({ on, onClick, children, role = "radio", disabled }: { on: boolean; onClick: () => void; children: ReactNode; role?: "radio" | "checkbox"; disabled?: boolean }) {
  return (
    <button
      type="button"
      role={role}
      aria-checked={on}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "rounded-pill px-3 py-1 text-meta font-bold ring-1 transition-colors disabled:opacity-40",
        on ? "bg-action text-white ring-action" : "bg-raised text-t2 ring-line hover:text-t1"
      )}
    >
      {children}
    </button>
  );
}

/** 色板：预设颜色 + 自选 + 用默认。值是 `#rrggbb` 或 null。 */
export function ColorPicker({ value, onChange, label }: { value: string | null; onChange: (v: string | null) => void; label: string }) {
  return (
    <div className="flex flex-wrap items-center gap-1.5" role="radiogroup" aria-label={label}>
      <Pill on={value === null} onClick={() => onChange(null)}>默认</Pill>
      {SWATCH_TOKENS.map((token) => {
        const hex = resolveToken(token);
        if (!hex) return null;
        const on = value === hex;
        return (
          <button
            key={token}
            type="button"
            role="radio"
            aria-checked={on}
            aria-label={hex}
            onClick={() => onChange(hex)}
            className={cn("size-7 rounded-circle ring-2 ring-offset-2 ring-offset-raised transition-transform hover:scale-110", on ? "ring-action" : "ring-transparent")}
            style={{ background: `var(${token})` }}
          />
        );
      })}
      <label className="relative grid size-7 cursor-pointer place-items-center rounded-circle bg-app text-micro font-bold text-t2 ring-1 ring-line" title="自己调颜色">
        +
        <input type="color" value={value ?? resolveToken(SWATCH_TOKENS[0]) ?? undefined} onChange={(e) => onChange(e.target.value.toLowerCase())} className="absolute inset-0 cursor-pointer opacity-0" aria-label={`${label}：自己调`} />
      </label>
      {value && <span className="font-mono text-micro text-t3">{value}</span>}
    </div>
  );
}
